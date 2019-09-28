use subprocess::Exec;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct OVSSwitch {
    name: String,
    ports: Vec<String>
}

impl OVSSwitch {
    pub fn create(name:&str) -> Option<OVSSwitch> {
        let output = Exec::shell(format!("ovs-vsctl add-br rs-{}",name)).capture().ok()?;
        if output.exit_status.success() {
            Some(OVSSwitch {
                name: name.to_string(),
                ports: vec![]
            })
        }
        else {
            print!("{}", output.stderr_str());
            None
        }
    }

//    pub fn get(name:&str) -> Option<OVSSwitch> {
//
//    }

    pub fn del(name:&str) {
        Exec::shell(format!("ovs-vsctl del-br rs-{}",name)).capture().unwrap();
    }

    pub fn add_port(&mut self, port_name:&str) {
        let output = Exec::shell(format!("ovs-vsctl add-port rs-{} {}",self.name,port_name)).capture().ok().unwrap();
        if output.exit_status.success() {
            self.ports.push(port_name.to_string());
        }
        else {
            print!("{}", output.stderr_str());
        }
    }

    pub async fn set_controller(&self, socket_addr:SocketAddr) {
        let output = tokio_process::Command::new("ovs-vsctl")
            .arg("set-controller")
            .arg(format!("rs-{}", self.name))
            .arg(format!("tcp:{}", socket_addr))
            .spawn().unwrap().await.unwrap();
        dbg!(output);
    }
}