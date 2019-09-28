use rtnetlink::new_connection;
use subprocess::Exec;
use crate::netns::NetNamespace;
use crate::net::{Net, TrafficControl};
use std::net::{Ipv4Addr, SocketAddr};
use rustyline::Editor;
use rustyline::error::ReadlineError;
use std::path::PathBuf;
use structopt::StructOpt;
use futures::{TryFutureExt, StreamExt, FutureExt};
use tokio::codec::LinesCodec;
use std::process::Stdio;
use std::io::Stdin;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod netns;
pub mod net;
pub mod switch;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    // A flag, true if used in the command line. Note doc comment will
    // be used for the help message of the flag. The name of the
    // argument will be, by default, based on the name of the field.
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,

    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Set speed
    #[structopt(short, long, default_value = "42")]
    speed: f64,

    /// Output file
    #[structopt(short, long, parse(from_os_str))]
    output: PathBuf,

    // the long option will be translated by default to kebab case,
    // i.e. `--nb-cars`.
    /// Number of cars
    #[structopt(short = "c", long)]
    nb_cars: Option<i32>,

    /// admin_level to consider
    #[structopt(short, long)]
    level: Vec<String>,

    /// Files to process
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(),Box<dyn std::error::Error>> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let mut net = Net {
        nss: vec![],
        host: vec![],
        ovsswitch: vec![],
        rtnetlink_handle: handle
    };
    net.clean().await;
    let cloud = net.add_host("cloud").unwrap();
    let edge = net.add_host("edge").unwrap();
    let device = net.add_host("d1").unwrap();

    let mut s1 = net.add_switch("s1").unwrap();
    let mut s2 = net.add_switch("s2").unwrap();
//    s1.set_controller("127.0.0.1:6653".parse().unwrap()).await;
//    s2.set_controller("127.0.0.1:6653".parse().unwrap()).await;
    net.connect_switch_host(&mut s1, &cloud, "1", Some((Ipv4Addr::from([10,0,0,1]),Ipv4Addr::from([255,255,255,0])))).await;
    net.connect_switch_host(&mut s1, &edge, "2", Some((Ipv4Addr::from([10,0,0,2]),Ipv4Addr::from([255,255,255,0])))).await;
    net.connect_switch_host(&mut s2, &edge, "3", Some((Ipv4Addr::from([10,0,1,1]),Ipv4Addr::from([255,255,255,0])))).await;
    let link = net.connect_switch_host(&mut s2, &device, "4", Some((Ipv4Addr::from([10,0,1,2]),Ipv4Addr::from([255,255,255,0])))).await;
//    link.two.add_tc(&TrafficControl {
//        bandwidth: 10.0,
//        delay: 0
//    });
    let mut rl = Editor::<()>::new();
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(ref line) if line.starts_with("xterm") => {
                rl.add_history_entry(line.as_str());
                let cmds:Vec<&str> = line.split(' ').collect();
                let hosts = &cmds[1..];
                for host in hosts {
                    let mut child = tokio_process::Command::new("ip");
                    child.arg("netns")
                        .arg("exec")
                        .arg(format!("rs-host-{}",host))
                        .arg("konsole");
                    let mut child = child.spawn().unwrap();
                    tokio::spawn(async {
                        let _ = child.await;
                    });
                }
            },
            Ok(line) => {

            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
    rl.save_history("history.txt").unwrap();
    net.clean().await;
    Ok(())
}
