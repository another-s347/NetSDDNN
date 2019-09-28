use crate::netns::NetNamespace;
use crate::switch::OVSSwitch;
use rtnetlink::Handle;
use std::panic::resume_unwind;
use subprocess::{Exec, CaptureData};
use std::net::{IpAddr, Ipv4Addr};
use futures::TryStreamExt;
use netlink_packet_route::rtnl::link::nlas::LinkNla;

pub struct Net {
    pub nss: Vec<NetNamespace>,
    pub host: Vec<Host>,
    pub ovsswitch: Vec<OVSSwitch>,
    pub rtnetlink_handle: Handle,
}

impl Net {
    pub fn add_host(&mut self, name: &str) -> Option<Host> {
        let netns_name = format!("rs-host-{}", name);
        let netns = NetNamespace::get_or_add(&netns_name)?;
        self.nss.push(netns.clone());
        let host = Host::new(name.to_string(), Some(netns));
        self.host.push(host.clone());
        Some(host)
    }

    pub fn add_switch(&mut self, name: &str) -> Option<OVSSwitch> {
        let s = OVSSwitch::create(name)?;
        self.ovsswitch.push(s.clone());
        Some(s)
    }

    pub async fn connect_switch_host(&mut self, s: &mut OVSSwitch, h: &Host, name: &str, ip: Option<(Ipv4Addr,Ipv4Addr)>) -> VethPair {
        let mut link = self.add_veth_link(name, None, ip).await.unwrap();
        s.add_port(&link.one.name);
        if let Some(ns) = h.netns.as_ref() {
            link.two.move_to_netns(ns);
        }
        link.one.up();
        link
    }

    pub async fn add_veth_link(&mut self, name: &str, one: Option<(Ipv4Addr,Ipv4Addr)>, two: Option<(Ipv4Addr,Ipv4Addr)>) -> Option<VethPair> {
        let result = self.rtnetlink_handle
            .link()
            .add()
            .veth(format!("rs-veth-{}-1", name), format!("rs-veth-{}-2", name))
            .execute()
            .await;
        if let Err(err) = result {
            println!("Err when add_veth_link: {:?}", err);
            None
        } else {
            Some(VethPair {
                one: Intf {
                    name: format!("rs-veth-{}-1", name),
                    intf_type: IntfType::VethPair,
                    netns: None,
                    ip: one
                },
                two: Intf { name: format!("rs-veth-{}-2", name), intf_type: IntfType::VethPair, netns: None, ip: two },
            })
        }
    }

    pub async fn clean(&self) {
        let mut links = self.rtnetlink_handle.link().get().execute();
        while let Ok(Some(l)) = links.try_next().await {
            for nla in l.nlas.iter() {
                if let LinkNla::IfName(s) = nla {
                    if s.starts_with("rs") {
                        self.rtnetlink_handle.link().del(l.header.index).execute().await;
                    }
                }
            }
        }
        let outputs = Exec::shell("ovs-vsctl list-br").capture().ok().unwrap().stdout_str();
        let brs:Vec<&str> = outputs.split('\n').collect();
        for br in brs {
            if br.starts_with("rs-") {
                Exec::shell(format!("ovs-vsctl del-br {}",br)).capture();
            }
        }
        for ns in self.nss.iter() {
            ns.del();
        }
    }
}

#[derive(Clone)]
pub struct Host {
    pub name: String,
    pub netns: Option<NetNamespace>,
}

impl Host {
    pub fn new(name: String, netns: Option<NetNamespace>) -> Host {
        // lo up
        let mut loopback = Intf {
            name: "lo".to_string(),
            intf_type: IntfType::Loopback,
            netns: netns.clone(),
            ip: Some((Ipv4Addr::from([127, 0, 0, 1]), Ipv4Addr::from([255, 255, 255, 255]))),
        };
        loopback.up();
        Host {
            name,
            netns,
        }
    }
}

pub struct VethPair {
    pub one: Intf,
    pub two: Intf,
}

pub struct Intf {
    pub name: String,
    pub intf_type: IntfType,
    pub netns: Option<NetNamespace>,
    pub ip: Option<(Ipv4Addr, Ipv4Addr)>,
}

impl Intf {
    pub fn move_to_netns(&mut self, netns: &NetNamespace) {
        if let Some(old) = self.netns.replace(netns.clone()) {
            let o = old.exec_shell(format!("ip link set {} netns {}", self.name, netns.name));
            handle_output(o, "del old netns");
        }
        let o = Exec::shell(format!("ip link set {} netns {}", self.name, netns.name)).capture().ok().unwrap();
        handle_output(o, "move to netns");
        self.set_ip();
        self.up();
        self.set_route_table();
    }

    pub fn set_ip(&self) {
        if let Some((ip, _)) = self.ip.as_ref() {
            let o = if let Some(netns) = self.netns.as_ref() {
                netns.exec_shell(format!("ip address add {} dev {}", ip, self.name))
            } else {
                Exec::shell(format!("ip address add {} dev {}", ip, self.name)).capture().ok().unwrap()
            };
            handle_output(o, "set ip");
        }
    }

    pub fn set_route_table(&self) {
        if let Some((ip, mask)) = self.ip.as_ref() {
            if let Some(netns) = self.netns.as_ref() {
                let ip = ip.octets();
                let _m = mask.octets();
                let masked_ip = Ipv4Addr::new(
                    ip[0]&_m[0],
                    ip[1]&_m[1],
                    ip[2]&_m[2],
                    ip[3]&_m[3],
                );
                let o = netns.exec_shell(format!("route add -net {} netmask {} dev {}", masked_ip, mask, self.name));
                handle_output(o, "set route table");
            }
        }
    }

    pub fn up(&mut self) {
        let o = if let Some(netns) = self.netns.as_ref() {
            netns.exec_shell(format!("ip link set {} up", self.name))
        } else {
            Exec::shell(format!("ip link set {} up", self.name)).capture().ok().unwrap()
        };
        handle_output(o, "set intf up");
    }

    /*
    tc qdisc add dev s1-eth3 root handle 5:0 htb default 1
    tc class add dev s1-eth3 parent 5:0 classid 5:1 htb rate 50.000000Mbit burst 15k
    tc qdisc add dev s1-eth3  parent 5:1  handle 10: netem delay 10
    */
    pub fn add_tc(&self, tc:&TrafficControl) {
        if let Some(netns) = self.netns.as_ref() {
            netns.exec_shell(format!("tc qdisc add dev {} root handle 5:0 htb default 1", self.name));
            netns.exec_shell(format!("tc class add dev {} parent 5:0 classid 5:1 htb rate {}Mbit burst 15k", self.name, tc.bandwidth));
            if tc.delay!=0 {
                netns.exec_shell(format!("tc qdisc add dev {} parent 5:1  handle 10: netem delay {}", self.name, tc.delay));
            }
        }
    }
}

pub enum IntfType {
    VethPair,
    Loopback,
}

pub fn handle_output(o: CaptureData, when: &str) {
    if !o.success() {
        println!("Err when:{}, {}", when, o.stderr_str());
    }
}

pub struct TrafficControl {
    pub bandwidth: f64,
    pub delay: u64
}