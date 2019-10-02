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
    let tc = TrafficControl {
        bandwidth: 300.0,
        delay: 1
    };
    let cloud = net.add_host("cloud").unwrap();
    let cn1 = net.add_host("cn1").unwrap();
    let cn2 = net.add_host("cn2").unwrap();
    let cn3 = net.add_host("cn3").unwrap();
    let cn4 = net.add_host("cn4").unwrap();
    let ed1 = net.add_host("ed1").unwrap();
    let ed2 = net.add_host("ed2").unwrap();
    let h1 = net.add_host("h1").unwrap();
    let h2 = net.add_host("h2").unwrap();

    let mut s0 = net.add_switch("s0").unwrap();
    let mut s1 = net.add_switch("fw1").unwrap();
    let mut s2 = net.add_switch("fw2").unwrap();
    let mut s3 = net.add_switch("fw3").unwrap();
    let mut s4 = net.add_switch("fw4").unwrap();
    let mut s5 = net.add_switch("fw5").unwrap();

    s1.set_controller("172.17.0.2:6653".parse().unwrap()).await;
    s2.set_controller("172.17.0.2:6653".parse().unwrap()).await;
    s3.set_controller("172.17.0.2:6653".parse().unwrap()).await;
    s4.set_controller("172.17.0.2:6653".parse().unwrap()).await;
    s5.set_controller("172.17.0.2:6653".parse().unwrap()).await;

    net.connect_switch_host(&mut s0, &cloud, "1", Some((Ipv4Addr::from([10,0,1,1]),Ipv4Addr::from([255,255,255,0])))).await;
    net.connect_switch_host(&mut s0, &ed1, "2", Some((Ipv4Addr::from([10,0,1,2]),Ipv4Addr::from([255,255,255,0])))).await;
    net.connect_switch_host(&mut s0, &ed2, "3", Some((Ipv4Addr::from([10,0,1,3]),Ipv4Addr::from([255,255,255,0])))).await;

    net.connect_switch_host(&mut s1,&ed1,"4", Some((Ipv4Addr::from([10,0,0,9]),Ipv4Addr::from([255,255,255,0])))).await;
    net.connect_switch_host(&mut s2,&ed2,"5", Some((Ipv4Addr::from([10,0,0,10]),Ipv4Addr::from([255,255,255,0])))).await;

    net.connect_switch_host(&mut s3,&cn1,"6", Some((Ipv4Addr::from([10,0,0,5]),Ipv4Addr::from([255,255,255,0])))).await.add_tc(&tc);
    net.connect_switch_host(&mut s4,&cn2,"7", Some((Ipv4Addr::from([10,0,0,6]),Ipv4Addr::from([255,255,255,0])))).await.add_tc(&tc);

    net.connect_switch_host(&mut s3, &h1, "h1s1",Some((Ipv4Addr::from([10,0,0,1]),Ipv4Addr::from([255,255,255,0])))).await.add_tc(&tc);
    net.connect_switch_host(&mut s4, &h2, "h2s2",Some((Ipv4Addr::from([10,0,0,2]),Ipv4Addr::from([255,255,255,0])))).await.add_tc(&tc);

    net.connect_switches(&mut s1,&mut s2,"10").await.add_tc(&tc);
    net.connect_switches(&mut s1,&mut s3,"11").await.add_tc(&tc);
    net.connect_switches(&mut s2,&mut s4,"12").await.add_tc(&tc);
    net.connect_switches(&mut s3,&mut s4,"13").await.add_tc(&tc);

    net.connect_switches(&mut s1,&mut s5,"14").await.add_tc(&tc);
    net.connect_switches(&mut s2,&mut s5,"15").await.add_tc(&tc);
    net.connect_switches(&mut s3,&mut s5,"16").await.add_tc(&tc);
    net.connect_switches(&mut s4,&mut s5,"17").await.add_tc(&tc);
//    let link = net.connect_switch_host(&mut s2, &device, "4", Some((Ipv4Addr::from([10,0,1,2]),Ipv4Addr::from([255,255,255,0])))).await;
//    link.two.add_tc(&TrafficControl {
//        bandwidth: 200.0,
//        delay: 10
//    });
    let mut rl = Editor::<()>::new();
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(ref line) if line.starts_with("shell") => {
                rl.add_history_entry(line.as_str());
                let cmds:Vec<&str> = line.split(' ').collect();
                let hosts = &cmds[1..];
                for host in hosts {
                    let mut child = tokio_process::Command::new("ip");
                    child
                        .current_dir("/home/skye/SDDNN/Model")
                        .arg("netns")
                        .arg("exec")
                        .arg(format!("rs-host-{}",host))
                        .arg("dbus-launch")
                        .arg("gnome-terminal")
                        .args(&["-t",host])
                        .arg("-e")
                        .arg("env TERM=ansi LD_LIBRARY_PATH=/home/skye/libtorch/lib:$LD_LIBRARY_PATH bash")
                        .arg("--wait")
                        .arg("-q");
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
