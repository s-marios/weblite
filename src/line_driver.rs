use byteorder::{BigEndian, ByteOrder};
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

    fn to_utf8(utf16: &str) -> String {
        //size checks
        let len = utf16.len();
        assert!(len > 0);
        assert_eq!(len % 2, 0);
        let size_for_u16 = len / 2;
        //setup accepting buffer
        let mut dst = Vec::<u16>::with_capacity(size_for_u16);
        for _ in 0..size_for_u16 {
            dst.push(0);
        }
        //read from the u16 buffer
        BigEndian::read_u16_into(utf16.as_bytes(), &mut dst);
        String::from_utf16_lossy(&dst)
    }

    ///execute a command using the backend
    pub fn exec(&mut self, command: &str) -> io::Result<String> {
        self.send(command)?;
        let mut response = String::new();
        let _len = self.r.read_line(&mut response)?;
        let response = LineDriver::to_utf8(&response).trim().to_string();
        Ok(response)
    }

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
                        responses.push(LineDriver::to_utf8(&response))
                    }
                }
            };
        }
        Ok(responses)
    }
}
