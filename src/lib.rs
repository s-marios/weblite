use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::io::{self, ErrorKind};
use std::path::Path;

mod converters;
pub mod descriptions;
pub mod echoinfo;
pub mod hex;
pub mod line_driver;
mod lineproto;

use converters::{AdditionalInfo, Converter, PropertyInfo};
pub use descriptions::*;
use echoinfo::{DeviceInfo, EchonetDevice, EchonetProperty};
pub use hex::*;
use line_driver::LineDriver;
use lineproto::LineResponse;

#[derive(Debug)]
pub struct AppData {
    pub config: Config,
    pub instances: Vec<EchonetDevice>,
    pub updated_descriptions: HashMap<String, DeviceDescription>,
    //pub descriptions: Descriptions,
    //pub superclass_dd: DeviceDescription,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub backend: String,
    pub dd_dir: String,
    pub superclass_dd: String,
    pub ai_file: String,
}

pub fn init() -> std::io::Result<AppData> {
    let config = init_config()?;
    let mut driver = LineDriver::from(&config.backend)?;
    let superclass_dd = read_def(&config.superclass_dd)?;
    let available_descriptions = read_device_descriptions(&config.dd_dir)?;
    let ais = converters::read_ais(&config.ai_file)?;
    println!("available device descriptions:");
    available_descriptions
        .iter()
        .for_each(|dd| println!("type (class): {} ({})", dd.device_type, dd.eoj));

    let discovered_classes = lineproto::get_all_classes(&mut driver)?;
    println!("discovered classes: {:?}", discovered_classes);

    //get all available device description classes
    let available_classes = available_descriptions
        .iter()
        .map(|dd| dd.eoj[2..].to_string())
        .collect::<HashSet<String>>();

    let classes = lineproto::class_intersect(&available_classes, &discovered_classes);
    //here we know which classes we have, generate converters?
    //1. filter available descriptions
    println!("classes to search for: {:?}\n", classes);
    let descriptions = available_descriptions
        .into_iter()
        .filter(|desc| classes.contains(&desc.eoj))
        //add the superclass thingie (copy, will be used later)
        .chain(std::iter::once(superclass_dd.clone()))
        .collect::<Vec<DeviceDescription>>();

    //2. for the above descriptions, generate Converters
    //TODO enter clone-city.. (fix this)
    let prop_sets = fuse_them(descriptions.clone(), ais);
    println!("\nEOJ: converters");
    prop_sets.iter().for_each(|(name, props)| {
        print!("conv: {}, props: ", name);
        props.iter().for_each(|prop| {
            print!("{},", prop.epc);
        });
        println!();
    });

    let scanned_devices = lineproto::scan_classes(classes, &mut driver);

    let devices = scanned_devices
        .into_iter()
        .map(|(_, v)| v)
        .collect::<Vec<DeviceInfo>>();
    //"instantiate" devices
    println!("\nInstances:--------\n");
    let instances = instantiate_devices(devices, prop_sets);
    instances.iter().for_each(|inst| {
        println!(
            "instance: {} {} props: {}",
            inst.host,
            inst.eoj,
            inst.properties.len()
        )
    });

    //for each instantiated device, generate an updated device description.
    //we'll have to use the device descriptions array, the superclass device description
    //and the instances
    let updated_descriptions =
        generate_updated_device_descriptions(descriptions, superclass_dd, &instances);

    Ok(AppData {
        config,
        instances,
        updated_descriptions,
        //descriptions,
        //superclass_dd,
    })
}

fn generate_updated_device_descriptions(
    descriptions: Vec<DeviceDescription>,
    superclass_dd: DeviceDescription,
    instances: &[EchonetDevice],
) -> HashMap<String, DeviceDescription> {
    instances
        .iter()
        .filter_map(|instance| {
            Some((
                instance,
                descriptions
                    .iter()
                    .find(|d| instance.eoj.starts_with(&d.eoj))?,
            ))
        })
        .map(|(instance, description)| adjust(instance, description, &superclass_dd))
        .collect()
}

fn adjust(
    instance: &EchonetDevice,
    description: &DeviceDescription,
    superclass_dd: &DeviceDescription,
) -> (String, DeviceDescription) {
    let mut adjusted_dd = description.clone();
    adjusted_dd.properties = adjusted_dd
        .properties
        .into_iter() //properties iterated
        .chain(superclass_dd.properties.clone().into_iter()) //and chain the superclass properties..
        .filter(|(_name, prop)| {
            instance
                .properties
                .iter()
                .any(|iprop| iprop.epc == prop.epc)
        })
        .collect();
    //set the eoj to the instance eoj
    adjusted_dd.eoj = instance.eoj.clone();
    (instance.hosteoj(), adjusted_dd)
}

fn instantiate_devices(
    devices: Vec<DeviceInfo>,
    prop_sets: HashMap<String, Vec<EchonetProperty>>,
) -> Vec<EchonetDevice> {
    devices
        .into_iter()
        .map(|di| {
            //chop the instance bit and get the device properties
            let mut properties = prop_sets.get(&di.eoj[..6]).unwrap().clone();
            //..and add the generic stuff
            properties.extend(prop_sets.get("0x00").unwrap().clone());
            print!("plen: {} ", properties.len());
            EchonetDevice::combine(di, properties)
        })
        .inspect(|ed| println!("After: h{} eoj{}\n   {:?}\n", ed.host, ed.eoj, ed))
        .collect()
}

fn fuse_them(
    descriptions: Vec<DeviceDescription>,
    ais: converters::Ais,
    //key is eoj
) -> HashMap<String, Vec<EchonetProperty>> {
    let mut map = HashMap::new();
    for dd in descriptions {
        //println!("fuse_them: dd {}", dd.eoj);
        //0. set up an array
        let mut props = vec![];
        let dd_ais = ais.properties(&dd.eoj);
        for (name, prop) in dd.properties {
            //println!("inner: {}, {:?}", name, prop);
            let conv_res = if let Some(ai) = get_ai(&prop.epc, dd_ais) {
                converters::fuse(&prop.schema, ai)
            //we have an ai, fuse!
            } else {
                //attempt to advance to a converter on its own
                Converter::try_from(prop.schema.clone())
            };

            match conv_res {
                Ok(conv) => {
                    let prop = EchonetProperty::new(name, prop.epc, prop.writable, conv);
                    props.push(prop);
                }
                Err(err) => {
                    println!(
                        "* Conversion Error! eoj/prop: {}/{} {}, consider Additional Info",
                        dd.eoj, prop.epc, err
                    );
                }
            }
        }
        map.insert(dd.eoj, props);
    }
    map
}

fn get_ai<'a>(dd: &str, ai: Option<&'a Vec<PropertyInfo>>) -> Option<&'a AdditionalInfo> {
    ai?.iter().find(|i| i.epc == dd).map(|i| &i.info)
}

pub fn init_config() -> std::io::Result<Config> {
    let contents = std::fs::read_to_string("./config.json")?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn init_device_descriptions<P: AsRef<Path>>(dir: P) -> std::io::Result<Descriptions> {
    read_device_descriptions(dir)
}

pub async fn versions() -> impl Responder {
    HttpResponse::Ok().body("TODO: elapi versions here")
}
pub async fn resources() -> impl Responder {
    HttpResponse::Ok().body("TODO: resources here")
}
pub async fn devices() -> impl Responder {
    HttpResponse::Ok().body("TODO: devices here")
}

pub async fn controllers() -> impl Responder {
    HttpResponse::Ok().body("TODO: controllers here")
}

pub async fn device(info: web::Path<String>, data: web::Data<AppData>) -> impl Responder {
    to_response_string(device_inner(info, data).await)
}

async fn device_inner(
    device: web::Path<String>,
    data: web::Data<AppData>,
) -> Result<String, NetError> {
    Ok(serde_json::to_string_pretty(
        &data
            .updated_descriptions
            .iter()
            .find(|(dev, _desc)| &device.as_ref() == dev)
            .ok_or_else(|| NetError::NoDevice)?
            .1,
    )
    .map_err(|internal| NetError::Internal(internal.to_string()))?)
}

pub async fn properties(_info: web::Path<String>, _data: web::Data<AppData>) -> impl Responder {
    HttpResponse::Ok().body("TODO: properties here")
}

async fn get_property_inner(
    info: web::Path<(String, String)>,
    data: web::Data<AppData>,
) -> Result<serde_json::Value, NetError> {
    let dev = get_device(&data.instances, &info.0).ok_or_else(|| NetError::NoDevice)?;
    let prop = get_dev_prop(&dev, &info.1).ok_or_else(|| NetError::NoProperty)?;
    let mut line = get_line(&data.config.backend)?;
    read_property(&mut line, dev, prop).await
}

pub async fn get_property(
    info: web::Path<(String, String)>,
    data: web::Data<AppData>,
) -> impl Responder {
    to_response(get_property_inner(info, data).await)
}

fn get_line(backend: &str) -> Result<LineDriver, NetError> {
    line_driver::LineDriver::from(&backend).map_err(|_| NetError::NoBackend)
}

#[derive(Debug)]
pub enum NetError {
    //internal errors
    Internal(String),
    Comm(String),
    NoBackend,
    NoResponse,
    ReadNG,
    WriteNG,
    Conversion(String),
    //404s
    NoProperty,
    NoDevice,
    //400
    BadRequest(String),
    Range(String),
}

impl NetError {
    fn error(&self) -> ErrorResponse {
        match self {
            NetError::NoDevice => ErrorResponse::new("referenceError".to_string())
                .with_message("No such device!".to_string()),
            NetError::NoProperty => ErrorResponse::new("referenceError".to_string())
                .with_message("No such property!".to_string()),
            NetError::NoResponse => ErrorResponse::new("timeoutError".to_string()),
            NetError::ReadNG => ErrorResponse::new("deviceError".to_string()),
            NetError::WriteNG => ErrorResponse::new("deviceError".to_string()),
            NetError::Comm(comm_error) => ErrorResponse::new("communicationError".to_string())
                .with_message(comm_error.clone()),
            NetError::BadRequest(details) => {
                ErrorResponse::new("typeError".to_string()).with_message(details.clone())
            }
            NetError::Range(details) => {
                ErrorResponse::new("rangeError".to_string()).with_message(details.clone())
            }
            NetError::Internal(details) => {
                ErrorResponse::new("internalError".to_string()).with_message(details.clone())
            }
            _ => ErrorResponse::new("internalError".to_string()),
        }
    }

    fn to_response(&self) -> actix_web::web::HttpResponse {
        match self {
            NetError::NoBackend => {
                HttpResponse::InternalServerError().body("no backend".to_string())
            }

            NetError::NoResponse => HttpResponse::InternalServerError().json(self.error()),
            NetError::Comm(err) => {
                HttpResponse::InternalServerError().body(format!("communications error: {}", err))
            }
            NetError::Conversion(err) => {
                HttpResponse::InternalServerError().body(format!("conversion error: {}", err))
            }
            NetError::NoDevice => HttpResponse::NotFound().json(self.error()),
            NetError::NoProperty => HttpResponse::NotFound().json(self.error()),
            NetError::BadRequest(ref _details) => HttpResponse::BadRequest().json(self.error()),
            NetError::Range(ref _details) => HttpResponse::BadRequest().json(self.error()),
            NetError::ReadNG => HttpResponse::MethodNotAllowed().json(self.error()),
            NetError::WriteNG => HttpResponse::MethodNotAllowed().json(self.error()),
            NetError::Internal(ref _details) => {
                HttpResponse::InternalServerError().json(self.error())
            }
        }
    }
}

#[derive(Serialize, Debug)]
struct ErrorResponse {
    #[serde(rename = "type")]
    error_type: String,
    message: Option<String>,
}

impl ErrorResponse {
    fn new(error_type: String) -> Self {
        ErrorResponse {
            error_type,
            message: None,
        }
    }

    fn with_message(self, message: String) -> Self {
        ErrorResponse {
            message: Some(message),
            ..self
        }
    }
}

async fn read_property(
    line: &mut LineDriver,
    dev: &EchonetDevice,
    prop: &EchonetProperty,
) -> Result<serde_json::Value, NetError> {
    //generate command
    let command = format!("{}:{}", dev.hosteoj(), prop.epc);
    let res = line
        .exec(&command)
        .map_err(|err| NetError::Comm(err.to_string()))?;
    let lr = LineResponse::try_from(&res as &str)
        .map_err(|_| NetError::Comm("Bad Line Response".to_string()))?;
    //check response status & if data is available
    //did we get an NG?
    if !lr.is_ok() {
        return Err(NetError::ReadNG);
    }
    //we should have data by this point
    let data_str = lr
        .data
        .ok_or_else(|| NetError::Comm("No data".to_string()))?;
    let data_u8 =
        hex::to_bytes(data_str).ok_or_else(|| NetError::Comm("Data-to-u8".to_string()))?;

    let mut context = data_u8[..].into();
    let value = prop.converter.convert_binary(&mut context)?;
    //we succeded! wrap up the value in a top level value and send it back up
    let mut map = serde_json::map::Map::with_capacity(1);
    map.insert(prop.name.clone(), value);
    Ok(serde_json::Value::Object(map))
}

fn get_device<'a>(devs: &'a [EchonetDevice], hosteoj: &str) -> Option<&'a EchonetDevice> {
    devs.iter().find(|dev| dev.hosteoj() == hosteoj)
}

fn get_dev_prop<'a>(dev: &'a EchonetDevice, name: &str) -> Option<&'a EchonetProperty> {
    dev.properties.iter().find(|prop| prop.name == name)
}

async fn set_property_inner(
    info: web::Path<(String, String)>,
    data: web::Data<AppData>,
    value: web::Json<serde_json::Value>,
) -> Result<serde_json::Value, NetError> {
    let dev = get_device(&data.instances, &info.0).ok_or_else(|| NetError::NoDevice)?;
    let prop = get_dev_prop(&dev, &info.1).ok_or_else(|| NetError::NoProperty)?;
    let mut line = get_line(&data.config.backend)?;
    //we've got to serialize there two... right?
    //I hope I'm right..
    let _ = write_property(&mut line, dev, prop, value).await?;
    read_property(&mut line, dev, prop).await
}

async fn write_property(
    line: &mut LineDriver,
    dev: &EchonetDevice,
    prop: &EchonetProperty,
    value: web::Json<serde_json::Value>,
) -> Result<serde_json::Value, NetError> {
    //we're receiving an object
    //get the the value of the property out of it
    let value = value.into_inner();
    let value = value
        .as_object()
        .ok_or_else(|| NetError::BadRequest("not an object!".to_string()))?;
    let value = value
        .get(&prop.name)
        .ok_or_else(|| NetError::BadRequest("no such value!".to_string()))?;
    //convert from value first

    let hex_data = hex::to_string(prop.converter.convert_json(value.clone())?);

    //generate command
    let command = format!("{}:{},{}", dev.hosteoj(), prop.epc, hex_data);
    println!("comand & hex_data: {}", command);
    //exec command
    let res = line
        .exec(&command)
        .map_err(|err| NetError::Comm(err.to_string()))?;
    let lr = LineResponse::try_from(&res as &str)
        .map_err(|_| NetError::Comm("Bad Line Response".to_string()))?;
    if lr.is_ok() {
        //this will be unused, but for signature's sake put something in
        Ok(serde_json::Value::Null)
    } else {
        Err(NetError::WriteNG)
    }
}

fn to_response(res: Result<serde_json::Value, NetError>) -> impl Responder {
    match res {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(error) => error.to_response(),
    }
}

fn to_response_string(res: Result<String, NetError>) -> impl Responder {
    match res {
        Ok(data) => HttpResponse::Ok().body(data),
        Err(error) => error.to_response(),
    }
}

pub async fn set_property(
    info: web::Path<(String, String)>,
    data: web::Data<AppData>,
    command: web::Json<serde_json::Value>,
) -> impl Responder {
    to_response(set_property_inner(info, data, command).await)
}

//This is the old ugly stuff, don't look at it too closely.
//TODO refarctor sometime
#[derive(Serialize, Deserialize, Debug)]
pub enum EchoCommand {
    #[serde(rename = "request")]
    Request {
        esv: String,
        operations: Vec<EchoOperation>,
    },
    #[serde(rename = "response")]
    Response {
        esv: String,
        operations: Vec<EchoOperation>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EchoOperation {
    epc: String,
    edt: Option<Vec<String>>,
}

fn do_command(line: &mut LineDriver, command: &str) -> io::Result<String> {
    line.exec(command)
}

fn generate_commands(
    device: String,
    service: &mut String,
    command: EchoCommand,
) -> Option<Vec<String>> {
    if let EchoCommand::Request { esv, operations } = command {
        *service = esv.clone();
        match esv.as_str() {
            //Get
            "0x62" => Some(operations.iter().fold(vec![], |mut v, op| {
                let mut cmd = String::new();
                cmd.push_str(&device);
                cmd.push_str(&format!(":{}", op.epc));
                cmd.push_str("\n");
                v.push(cmd);
                v
            })),
            //SetGet
            "0x6E" => unimplemented!(),
            _ => None,
        }
    } else {
        None
    }
}

fn response_to_echo_result(
    mut _service: String,
    responses: Vec<String>,
) -> Result<EchoCommand, io::Error> {
    //gather all the pieces of information...

    let mut esv = "0x72".to_string();
    let ops = responses
        .iter()
        .map(|response| {
            let mut tokens = response.split(',');
            let okng = tokens.next()?;
            //TODO: why this doesn't work with
            //starts_with, == etc? and only works with contains?!?
            if okng.contains("NG") {
                esv = "0x52".to_string();
            }

            let target = tokens.next()?;
            let epc = target.rsplit(':').next()?.to_string();
            let opt_data = tokens.next();

            let edt: Option<Vec<String>> = if let Some(data) = opt_data {
                Some(
                    data.chars()
                        .skip(2)
                        .collect::<Vec<char>>()
                        .chunks_exact(2)
                        .map(|chunk| chunk.iter().collect::<String>())
                        .map(|hex| String::from("0x") + &hex)
                        .collect::<Vec<String>>(),
                )
            } else {
                None
            };
            Some(EchoOperation { epc, edt })
        })
        .collect::<Option<Vec<EchoOperation>>>();

    match ops {
        None => Err(io::Error::new(
            ErrorKind::InvalidInput,
            "an op was malformed!",
        )),
        Some(operations) => Ok(EchoCommand::Response { esv, operations }),
    }
}

pub async fn echo_commands(
    device: web::Path<String>,
    command: web::Json<EchoCommand>,
    data: web::Data<AppData>,
) -> impl Responder {
    let connect = line_driver::LineDriver::from(&data.config.backend);
    match connect {
        Err(error) => format!("failed to connect: {}", error),
        Ok(mut line) => {
            let command = command.into_inner();
            let device = device.into_inner();

            let mut service = String::new();
            if let Some(commands) = generate_commands(device, &mut service, command) {
                let mut keep_going = true;
                let results = commands
                    .iter()
                    .map(|cmd| do_command(&mut line, cmd))
                    .try_fold(vec![], |mut acc, cmd_res| {
                        if !keep_going {
                            return Err("stop");
                        }
                        match cmd_res {
                            Ok(res) => {
                                keep_going = res.starts_with("OK");
                                acc.push(res);
                                Ok(acc)
                            }
                            _ => Err("transmition error"),
                        }
                    });

                match results {
                    Ok(strings) => serde_json::to_string_pretty(
                        &response_to_echo_result(service, strings).unwrap(),
                    )
                    .unwrap(),
                    Err(error) => error.to_string(),
                }
            } else {
                String::from("bad command")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
