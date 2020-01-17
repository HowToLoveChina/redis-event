/*!
[`RedisListener`]接口的具体实现

此模块包括：
- `standalone`, 处理单节点Redis事件
- `sentinel`, 处理sentinel模式Redis事件
- `cluster`, 处理cluster模式Redis事件

[`RedisListener`]: trait.RedisListener.html
*/
pub mod standalone {
    use std::borrow::Borrow;
    use std::io::{ErrorKind, Result};
    use std::net::TcpStream;
    use std::result::Result::Ok;
    use std::sync::{Arc, mpsc};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    
    use log::{error, info};
    
    use crate::{cmd, CommandHandler, io, NoOpCommandHandler, NoOpRdbHandler, rdb, RdbHandler, RedisListener, to_string};
    use crate::config::Config;
    use crate::io::{Conn, send};
    use crate::rdb::Data;
    use crate::rdb::Data::Bytes;
    
    /// 用于监听单个Redis实例的事件
    pub struct Listener {
        pub config: Config,
        conn: Option<Conn>,
        rdb_listener: Box<dyn RdbHandler>,
        cmd_listener: Box<dyn CommandHandler>,
        t_heartbeat: HeartbeatWorker,
        sender: Option<mpsc::Sender<Message>>,
        running: Arc<AtomicBool>,
    }
    
    impl Listener {
        fn connect(&mut self) -> Result<()> {
            let stream = TcpStream::connect(self.config.addr)?;
            stream.set_read_timeout(Option::Some(Duration::from_millis(self.config.read_timeout)))
                .expect("read timeout set failed");
            stream.set_write_timeout(Option::Some(Duration::from_millis(self.config.write_timeout)))
                .expect("write timeout set failed");
            info!("connected to server {}", self.config.addr.to_string());
            self.conn = Option::Some(io::new(stream));
            Ok(())
        }
        
        fn auth(&mut self) -> Result<()> {
            if !self.config.password.is_empty() {
                let conn = self.conn.as_mut().unwrap();
                conn.send(b"AUTH", &[self.config.password.as_bytes()])?;
                conn.reply(io::read_bytes, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())?;
            }
            Ok(())
        }
        
        fn send_port(&mut self) -> Result<()> {
            let conn = self.conn.as_mut().unwrap();
            let stream: &TcpStream = match conn.input.as_any().borrow().downcast_ref::<TcpStream>() {
                Some(stream) => stream,
                None => panic!("not tcp stream")
            };
            let port = stream.local_addr()?.port().to_string();
            let port = port.as_bytes();
            conn.send(b"REPLCONF", &[b"listening-port", port])?;
            conn.reply(io::read_bytes, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())?;
            Ok(())
        }
        
        pub fn set_rdb_listener(&mut self, listener: Box<dyn RdbHandler>) {
            self.rdb_listener = listener
        }
        
        pub fn set_command_listener(&mut self, listener: Box<dyn CommandHandler>) {
            self.cmd_listener = listener
        }
        
        fn start_sync(&mut self) -> Result<bool> {
            let offset = self.config.repl_offset.to_string();
            let repl_offset = offset.as_bytes();
            let repl_id = self.config.repl_id.as_bytes();
            
            let conn = self.conn.as_mut().unwrap();
            conn.send(b"PSYNC", &[repl_id, repl_offset])?;
            
            if let Bytes(resp) = conn.reply(io::read_bytes, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())? {
                let resp = to_string(resp);
                if resp.starts_with("FULLRESYNC") {
                    if self.config.is_discard_rdb {
                        conn.reply(io::skip, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())?;
                    } else {
                        conn.reply(rdb::parse, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())?;
                    }
                    let mut iter = resp.split_whitespace();
                    if let Some(repl_id) = iter.nth(1) {
                        self.config.repl_id = repl_id.to_owned();
                    } else {
                        panic!("Expect replication id, bot got None");
                    }
                    if let Some(repl_offset) = iter.next() {
                        self.config.repl_offset = repl_offset.parse::<i64>().unwrap();
                    } else {
                        panic!("Expect replication offset, bot got None");
                    }
                    return Ok(true);
                } else if resp.starts_with("CONTINUE") {
                    // PSYNC 继续之前的offset
                    let mut iter = resp.split_whitespace();
                    if let Some(repl_id) = iter.nth(1) {
                        if !repl_id.eq(&self.config.repl_id) {
                            self.config.repl_id = repl_id.to_owned();
                        }
                    }
                    return Ok(true);
                } else if resp.starts_with("NOMASTERLINK") {
                    // redis丢失了master
                    return Ok(false);
                } else if resp.starts_with("LOADING") {
                    // redis正在启动，加载rdb中
                    return Ok(false);
                } else {
                    // 不支持PSYNC命令，改用SYNC命令
                    conn.send(b"SYNC", &Vec::new())?;
                    if self.config.is_discard_rdb {
                        conn.reply(io::skip, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())?;
                    } else {
                        conn.reply(rdb::parse, self.rdb_listener.as_mut(), self.cmd_listener.as_mut())?;
                    }
                    return Ok(true);
                }
            } else {
                panic!("Expect Redis string response");
            }
        }
        
        fn receive_cmd(&mut self) -> Result<Data<Vec<u8>, Vec<Vec<u8>>>> {
            let conn = self.conn.as_mut().unwrap();
            conn.mark();
            let cmd = conn.reply(io::read_bytes, self.rdb_listener.as_mut(), self.cmd_listener.as_mut());
            let read_len = conn.unmark()?;
            self.config.repl_offset += read_len;
            if let Err(error) = self.sender.as_ref().unwrap().send(Message::Some(self.config.repl_offset)) {
                error!("repl offset send error: {}", error);
            }
            return cmd;
        }
        
        fn start_heartbeat(&mut self) {
            let conn = self.conn.as_ref().unwrap();
            let stream: &TcpStream = match conn.input.as_any().borrow().downcast_ref::<TcpStream>() {
                Some(stream) => stream,
                None => panic!("not tcp stream")
            };
            let mut stream_clone = stream.try_clone().unwrap();
            
            let (sender, receiver) = mpsc::channel();
            
            let t = thread::spawn(move || {
                let mut offset = 0;
                let mut timer = Instant::now();
                let half_sec = Duration::from_millis(500);
                info!("heartbeat thread started");
                loop {
                    match receiver.recv_timeout(half_sec) {
                        Ok(Message::Terminate) => break,
                        Ok(Message::Some(new_offset)) => {
                            offset = new_offset;
                        }
                        Err(_) => {}
                    };
                    let elapsed = timer.elapsed();
                    if elapsed.ge(&half_sec) {
                        let offset_str = offset.to_string();
                        let offset_bytes = offset_str.as_bytes();
                        if let Err(error) = send(&mut stream_clone, b"REPLCONF", &[b"ACK", offset_bytes]) {
                            error!("heartbeat error: {}", error);
                            break;
                        }
                        timer = Instant::now();
                    }
                }
                info!("heartbeat thread terminated");
            });
            self.t_heartbeat = HeartbeatWorker { thread: Some(t) };
            self.sender = Some(sender);
        }
    }
    
    impl RedisListener for Listener {
        fn open(&mut self) -> Result<()> {
            self.connect()?;
            self.auth()?;
            self.send_port()?;
            while !self.start_sync()? {
                sleep(Duration::from_secs(5));
            }
            if !self.config.is_aof {
                return Ok(());
            }
            self.start_heartbeat();
            while self.running.load(Ordering::Relaxed) {
                match self.receive_cmd() {
                    Ok(Data::Bytes(_)) => panic!("Expect BytesVec response, but got Bytes"),
                    Ok(Data::BytesVec(data)) => cmd::parse(data, self.cmd_listener.as_mut()),
                    Err(ref err) if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut => {
                        // 不管，连接是好的
                    }
                    Err(err) => return Err(err),
                    Ok(Data::Empty) => {}
                }
            }
            Ok(())
        }
    }
    
    impl Drop for Listener {
        fn drop(&mut self) {
            if let Some(sender) = self.sender.as_ref() {
                if let Err(err) = sender.send(Message::Terminate) {
                    error!("Closing heartbeat thread error: {}", err)
                }
            }
            if let Some(thread) = self.t_heartbeat.thread.take() {
                if let Err(_) = thread.join() {}
            }
        }
    }
    
    /// Listener实例的创建方法
    pub fn new(conf: Config, running: Arc<AtomicBool>) -> Listener {
        if conf.is_aof && !running.load(Ordering::Relaxed) {
            running.store(true, Ordering::Relaxed);
        }
        
        Listener {
            config: conf,
            conn: Option::None,
            rdb_listener: Box::new(NoOpRdbHandler {}),
            cmd_listener: Box::new(NoOpCommandHandler {}),
            t_heartbeat: HeartbeatWorker { thread: None },
            sender: None,
            running,
        }
    }
    
    struct HeartbeatWorker {
        thread: Option<thread::JoinHandle<()>>
    }
    
    enum Message {
        Terminate,
        Some(i64),
    }
}

pub mod cluster {}

pub mod sentinel {}