use subprocess::{Exec, CaptureData};
use crate::net::handle_output;

#[derive(Clone)]
pub struct NetNamespace {
    pub name: String
}

impl NetNamespace {
    pub fn add(name:&str) -> Option<NetNamespace> {
        let output = Exec::shell(format!("ip netns add {}",name)).capture().ok()?;
        if output.exit_status.success() {
            Some(NetNamespace {
                name: name.to_string()
            })
        }
        else {
            print!("{}", output.stderr_str());
            None
        }
    }

    pub fn get(name:&str) -> Option<NetNamespace> {
        let output = Exec::shell("ip netns show").capture().ok()?.stdout_str();
        let nss:Vec<&str> = output.split('\n').collect();
        if nss.iter().find(|x|x.contains(name)).is_some() {
            Some(NetNamespace {
                name: name.to_string()
            })
        }
        else {
            None
        }
    }

    pub fn get_or_add(name:&str) -> Option<NetNamespace> {
        Self::get(name).or(Self::add(name))
    }

    pub fn del(&self) {
        Exec::shell(format!("ip netns del {}", self.name)).capture().unwrap();
    }

    pub fn exec_shell(&self, cmd: String) -> CaptureData {
        Exec::shell(format!("ip netns exec {} {}",self.name, cmd)).capture().ok().unwrap()
    }
}