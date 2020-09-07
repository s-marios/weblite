use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::net::TcpStream;
use std::time::Duration;

pub(crate) struct LineDriver {
    r: BufReader<TcpStream>,
    w: BufWriter<TcpStream>,
}

impl LineDriver {
    pub fn new(host: &str, port: u16) -> io::Result<Self> {
        let con = TcpStream::connect((host, port))?;
        LineDriver::setup(con)
    }

    pub fn from(hostspec: &str) -> io::Result<Self> {
        let con = TcpStream::connect(hostspec)?;
        LineDriver::setup(con)
    }

    fn setup(con: TcpStream) -> io::Result<Self> {
        //setup timeout
        con.set_read_timeout(Some(Duration::from_millis(1500)))?;
        //clone so we can have buf reader & buf writer ends
        let clone = con.try_clone()?;

        Ok(LineDriver {
            r: BufReader::new(con),
            w: BufWriter::new(clone),
        })
    }

    fn send(&mut self, command: &str) -> io::Result<()> {
        writeln!(self.w, "{}", command)?;
        self.w.flush()?;
        Ok(())
    }

    ///execute a command using the backend
    pub fn exec(&mut self, command: &str) -> io::Result<String> {
        self.send(command)?;
        let mut response = String::new();
        let _len = self.r.read_line(&mut response)?;
        Ok(response.trim().to_string())
    }

    //TODO what's with the result that never fails? think about it
    ///execute a command, expect multiple replies
    pub fn exec_multi(&mut self, command: &str) -> io::Result<Vec<String>> {
        self.send(command)?;
        let mut responses = vec![];
        loop {
            let mut response = String::new();
            match self.r.read_line(&mut response) {
                Err(_) => break,
                Ok(len) => {
                    if len > 0 {
                        responses.push(response.trim().to_string())
                    }
                }
            };
        }
        Ok(responses)
    }
}
