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

    pub fn exec(&mut self, command: &str) -> io::Result<String> {
        writeln!(self.w, "{}", command)?;
        self.w.flush()?;
        let mut response = String::new();
        let _len = self.r.read_line(&mut response)?;
        Ok(response)
    }
}
