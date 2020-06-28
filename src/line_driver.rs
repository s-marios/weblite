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
        //setup timeout
        con.set_read_timeout(Some(Duration::from_millis(3500)))?;
        //clone so we can have buf reader & buf writer ends
        let clone = con.try_clone()?;

        Ok(LineDriver {
            r: BufReader::new(con),
            w: BufWriter::new(clone),
        })
    }

    ///execute a command using the backend
    pub fn exec(&mut self, command: &str) -> io::Result<String> {
        writeln!(self.w, "{}", command)?;
        self.w.flush()?;
        let mut response = String::new();
        let len = self.r.read_line(&mut response)?;
        //we have a line, convert this to utf-8
        assert_eq!(len % 2, 0);
        let size_for_u16 = len / 2;
        let mut dst = Vec::<u16>::with_capacity(size_for_u16);
        for _ in 0..size_for_u16 {
            dst.push(0);
        }

        BigEndian::read_u16_into(response.as_bytes(), &mut dst);
        let response = String::from_utf16_lossy(&dst);

        Ok(response)
    }
}
