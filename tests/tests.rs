use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

use serial_test::serial;

use redis_event::{CommandHandler, NoOpCommandHandler, RdbHandler, RedisListener};
use redis_event::config::Config;
use redis_event::listener::standalone;
use redis_event::rdb::{ExpireType, Object};

#[test]
#[serial]
fn test_hash() {
    struct TestRdbHandler {}
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Hash(hash) => {
                    let key = String::from_utf8_lossy(hash.key);
                    assert_eq!("force_dictionary", key);
                    for field in hash.fields {
                        assert_eq!(50, field.name.len());
                        assert_eq!(50, field.value.len());
                    }
                }
                _ => {}
            }
        }
    }
    
    start_redis_test("dictionary.rdb", Box::new(TestRdbHandler {}), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_hash_1() {
    struct TestRdbHandler {}
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Hash(hash) => {
                    let key = String::from_utf8_lossy(hash.key);
                    assert_eq!("zipmap_compresses_easily", key);
                    let mut map = HashMap::new();
                    for field in hash.fields {
                        let name = String::from_utf8_lossy(&field.name);
                        let value = String::from_utf8_lossy(&field.value);
                        map.insert(name, value);
                    }
                    assert_eq!("aa", map.get("a").unwrap());
                    assert_eq!("aaaa", map.get("aa").unwrap());
                    assert_eq!("aaaaaaaaaaaaaa", map.get("aaaaa").unwrap());
                }
                _ => {}
            }
        }
    }
    
    start_redis_test("hash_as_ziplist.rdb", Box::new(TestRdbHandler {}), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_string() {
    struct TestRdbHandler {}
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::String(kv) => {
                    let key = String::from_utf8_lossy(kv.key);
                    assert_eq!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", key);
                    assert_eq!("Key that redis should compress easily", String::from_utf8_lossy(kv.value))
                }
                _ => {}
            }
        }
    }
    
    start_redis_test("easily_compressible_string_key.rdb", Box::new(TestRdbHandler {}), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_integer() {
    struct TestRdbHandler {
        map: HashMap<String, String>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::String(kv) => {
                    self.map.insert(String::from_utf8_lossy(kv.key).to_string(),
                                    String::from_utf8_lossy(kv.value).to_string());
                }
                Object::EOR => {
                    assert_eq!(self.map.get("125").unwrap(), "Positive 8 bit integer");
                    assert_eq!(self.map.get("43947").unwrap(), "Positive 16 bit integer");
                    assert_eq!(self.map.get("183358245").unwrap(), "Positive 32 bit integer");
                    assert_eq!(self.map.get("-123").unwrap(), "Negative 8 bit integer");
                    assert_eq!(self.map.get("-29477").unwrap(), "Negative 16 bit integer");
                    assert_eq!(self.map.get("-183358245").unwrap(), "Negative 32 bit integer");
                }
                _ => {}
            }
        }
    }
    
    start_redis_test("integer_keys.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_intset16() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<String>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Set(set) => {
                    let key = String::from_utf8_lossy(set.key).to_string();
                    let mut val = Vec::new();
                    for mem in set.members {
                        val.push(String::from_utf8_lossy(mem).to_string());
                    }
                    self.map.insert(key, val);
                }
                Object::EOR => {
                    let values = self.map.get("intset_16").unwrap();
                    let arr = ["32766", "32765", "32764"];
                    for val in values {
                        assert!(arr.contains(&val.as_str()));
                    }
                }
                _ => {}
            }
        }
    }
    start_redis_test("intset_16.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_intset32() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<String>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Set(set) => {
                    let key = String::from_utf8_lossy(set.key).to_string();
                    let mut val = Vec::new();
                    for mem in set.members {
                        val.push(String::from_utf8_lossy(mem).to_string());
                    }
                    self.map.insert(key, val);
                }
                Object::EOR => {
                    let values = self.map.get("intset_32").unwrap();
                    let arr = ["2147418110", "2147418109", "2147418108"];
                    for val in values {
                        assert!(arr.contains(&val.as_str()));
                    }
                }
                _ => {}
            }
        }
    }
    start_redis_test("intset_32.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_intset64() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<String>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Set(set) => {
                    let key = String::from_utf8_lossy(set.key).to_string();
                    let mut val = Vec::new();
                    for mem in set.members {
                        val.push(String::from_utf8_lossy(mem).to_string());
                    }
                    self.map.insert(key, val);
                }
                Object::EOR => {
                    let values = self.map.get("intset_64").unwrap();
                    let arr = ["9223090557583032318", "9223090557583032317", "9223090557583032316"];
                    for val in values {
                        assert!(arr.contains(&val.as_str()));
                    }
                }
                _ => {}
            }
        }
    }
    start_redis_test("intset_64.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_keys_with_expiry() {
    struct TestRdbHandler {}
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::String(kv) => {
                    let key = String::from_utf8_lossy(kv.key).to_string();
                    let val = String::from_utf8_lossy(kv.value).to_string();
                    assert_eq!("expires_ms_precision", key);
                    assert_eq!("2022-12-25 10:11:12.573 UTC", val);
                    if let Some(ExpireType::Millisecond) = kv.meta.expired_type {
                        assert_eq!(1671963072573, kv.meta.expired_time.unwrap());
                    } else {
                        panic!("错误的过期类型")
                    }
                }
                _ => {}
            }
        }
    }
    start_redis_test("keys_with_expiry.rdb", Box::new(TestRdbHandler {}), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_linked_list() {
    struct TestRdbHandler {
        list: Vec<String>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::List(list) => {
                    assert_eq!("force_linkedlist", String::from_utf8_lossy(list.key));
                    for val in list.values {
                        let value = String::from_utf8_lossy(val).to_string();
                        self.list.push(value);
                    }
                }
                Object::EOR => {
                    assert_eq!(1000, self.list.len());
                    assert_eq!("41PJSO2KRV6SK1WJ6936L06YQDPV68R5J2TAZO3YAR5IL5GUI8", self.list.get(0).unwrap());
                    assert_eq!("E41JRQX2DB4P1AQZI86BAT7NHPBHPRIIHQKA4UXG94ELZZ7P3Y", self.list.get(1).unwrap());
                }
                _ => {}
            }
        }
    }
    start_redis_test("linkedlist.rdb", Box::new(TestRdbHandler { list: vec![] }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_multiple_database() {
    struct TestRdbHandler {}
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::String(kv) => {
                    let key = String::from_utf8_lossy(kv.key);
                    if "key_in_zeroth_database".eq(&key) {
                        assert_eq!(0, kv.meta.db);
                        assert_eq!("zero", String::from_utf8_lossy(kv.value))
                    } else if "key_in_second_database".eq(&key) {
                        assert_eq!(2, kv.meta.db);
                        assert_eq!("second", String::from_utf8_lossy(kv.value))
                    } else {
                        panic!("key名错误")
                    }
                }
                _ => {}
            }
        }
    }
    start_redis_test("multiple_databases.rdb", Box::new(TestRdbHandler {}), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_regular_set() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<String>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Set(set) => {
                    let key = String::from_utf8_lossy(set.key).to_string();
                    let mut val = Vec::new();
                    for mem in set.members {
                        val.push(String::from_utf8_lossy(mem).to_string());
                    }
                    self.map.insert(key, val);
                }
                Object::EOR => {
                    let values = self.map.get("regular_set").unwrap();
                    let arr = ["alpha", "beta", "gamma", "delta", "phi", "kappa"];
                    for val in values {
                        assert!(arr.contains(&val.as_str()));
                    }
                }
                _ => {}
            }
        }
    }
    start_redis_test("regular_set.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_regular_sorted_set() {
    struct TestRdbHandler {
        map: HashMap<String, f64>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::SortedSet(set) => {
                    let key = String::from_utf8_lossy(set.key).to_string();
                    assert_eq!("force_sorted_set", key);
                    for item in set.items {
                        self.map.insert(String::from_utf8_lossy(&item.member).to_string(), item.score);
                    }
                }
                Object::EOR => {
                    assert_eq!(500, self.map.len());
                    assert_eq!(3.19, self.map.get("G72TWVWH0DY782VG0H8VVAR8RNO7BS9QGOHTZFJU67X7L0Z3PR").unwrap().clone());
                    assert_eq!(0.76, self.map.get("N8HKPIK4RC4I2CXVV90LQCWODW1DZYD0DA26R8V5QP7UR511M8").unwrap().clone());
                }
                _ => {}
            }
        }
    }
    start_redis_test("regular_sorted_set.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_zipmap_big_values() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<u8>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Hash(hash) => {
                    assert_eq!("zipmap_with_big_values", String::from_utf8_lossy(hash.key));
                    for field in hash.fields {
                        let name = String::from_utf8_lossy(&field.name).to_string();
                        self.map.insert(name, field.value.to_vec());
                    }
                }
                Object::EOR => {
                    assert_eq!(253, self.map.get("253bytes").unwrap().len());
                    assert_eq!(254, self.map.get("254bytes").unwrap().len());
                    assert_eq!(255, self.map.get("255bytes").unwrap().len());
                    assert_eq!(300, self.map.get("300bytes").unwrap().len());
                    assert_eq!(20000, self.map.get("20kbytes").unwrap().len());
                }
                _ => {}
            }
        }
    }
    start_redis_test("zipmap_with_big_values.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_zipmap_compress() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<u8>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Hash(hash) => {
                    assert_eq!("zipmap_compresses_easily", String::from_utf8_lossy(hash.key));
                    for field in hash.fields {
                        let name = String::from_utf8_lossy(&field.name).to_string();
                        self.map.insert(name, field.value.to_vec());
                    }
                }
                Object::EOR => {
                    assert_eq!("aa", self.map.get("a").unwrap());
                    assert_eq!("aaaa", self.map.get("aa").unwrap());
                    assert_eq!("aaaaaaaaaaaaaa", self.map.get("aaaaa").unwrap());
                }
                _ => {}
            }
        }
    }
    start_redis_test("zipmap_that_compresses_easily.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_zipmap_not_compress() {
    struct TestRdbHandler {
        map: HashMap<String, Vec<u8>>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::Hash(hash) => {
                    assert_eq!("zimap_doesnt_compress", String::from_utf8_lossy(hash.key));
                    for field in hash.fields {
                        let name = String::from_utf8_lossy(&field.name).to_string();
                        self.map.insert(name, field.value.to_vec());
                    }
                }
                Object::EOR => {
                    assert_eq!("2", self.map.get("MKD1G6").unwrap());
                    assert_eq!("F7TI", self.map.get("YNNXK").unwrap());
                }
                _ => {}
            }
        }
    }
    start_redis_test("zipmap_that_doesnt_compress.rdb", Box::new(TestRdbHandler { map: HashMap::new() }), Box::new(NoOpCommandHandler {}));
}

#[test]
#[serial]
fn test_ziplist() {
    struct TestRdbHandler {
        list: Vec<String>
    }
    
    impl RdbHandler for TestRdbHandler {
        fn handle(&mut self, data: Object) {
            match data {
                Object::List(list) => {
                    assert_eq!("ziplist_compresses_easily", String::from_utf8_lossy(list.key));
                    for val in list.values {
                        let value = String::from_utf8_lossy(val).to_string();
                        self.list.push(value);
                    }
                }
                Object::EOR => {
                    assert_eq!(6, self.list.len());
                    assert_eq!("aaaaaa", self.list.get(0).unwrap());
                    assert_eq!("aaaaaaaaaaaa", self.list.get(1).unwrap());
                    assert_eq!("aaaaaaaaaaaaaaaaaa", self.list.get(2).unwrap());
                    assert_eq!("aaaaaaaaaaaaaaaaaaaaaaaa", self.list.get(3).unwrap());
                    assert_eq!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", self.list.get(4).unwrap());
                    assert_eq!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", self.list.get(5).unwrap());
                }
                _ => {}
            }
        }
    }
    start_redis_test("ziplist_that_compresses_easily.rdb", Box::new(TestRdbHandler { list: vec![] }), Box::new(NoOpCommandHandler {}));
}

fn start_redis_test(rdb: &str, rdb_handler: Box<dyn RdbHandler>, cmd_handler: Box<dyn CommandHandler>) {
    let port: u16 = 16379;
    let pid = start_redis_server(rdb, port);
    // wait redis to start
    sleep(Duration::from_secs(10));
    
    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let conf = Config {
        is_discard_rdb: false,
        is_aof: false,
        addr: SocketAddr::new(ip, port),
        password: String::new(),
        repl_id: String::from("?"),
        repl_offset: -1,
    };
    let mut redis_listener = standalone::new(conf);
    redis_listener.set_rdb_listener(rdb_handler);
    redis_listener.set_command_listener(cmd_handler);
    if let Err(error) = redis_listener.open() {
        panic!(error)
    }
    shutdown_redis(pid);
}

fn start_redis_server(rdb: &str, port: u16) -> u32 {
    // redis-server --port 6379 --daemonize no --dbfilename rdb --dir ./tests/rdb
    let child = Command::new("redis-server")
        .arg("--port")
        .arg(port.to_string())
        .arg("--daemonize")
        .arg("no")
        .arg("--dbfilename")
        .arg(rdb)
        .arg("--dir")
        .arg("./tests/rdb")
        .arg("--logfile")
        .arg("./redis.log")
        .spawn()
        .expect("failed to start redis-server");
    return child.id();
}

fn shutdown_redis(pid: u32) {
    let pid_str = format!("{}", pid);
    let output = Command::new("kill")
        .arg("-9")
        .arg(pid_str)
        .output()
        .expect("kill redis failed");
    println!("{:?}", output);
}