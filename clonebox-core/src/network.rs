use core::ffi::{c_int, c_uchar, c_uint, c_ushort};
use libc::{ifaddrmsg, ifinfomsg, nlmsghdr, rtattr};
use nix::{
    sched::{CloneFlags, setns},
    sys::socket::{
        AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType, bind, recv, send,
        socket,
    },
    unistd::Pid,
};
use std::fs::File;
use std::mem::size_of;
use std::net::Ipv4Addr;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

use crate::error::{CoreError, NamespaceError, NetworkError};

const VETH_INFO_PEER: u16 = 1;
const RTEXT_FILTER_VF: u32 = 1 << 0;
const RTEXT_FILTER_SKIP_STATS: u32 = 1 << 3;

pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    fn flush(&mut self) {
        self.buf.clear();
    }

    fn pad_to_align(&mut self) {
        //while self.buf.len() % 4 != 0
        while !self.buf.len().is_multiple_of(4) {
            self.buf.push(0);
        }
    }

    fn pos(&self) -> usize {
        self.buf.len()
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> usize {
        self.buf.extend_from_slice(bytes);

        bytes.len()
    }

    pub fn write_u8(&mut self, n: u8) -> usize {
        self.buf.extend_from_slice(&n.to_ne_bytes());

        size_of::<u8>()
    }

    pub fn write_u16(&mut self, n: u16) -> usize {
        self.buf.extend_from_slice(&n.to_ne_bytes());

        size_of::<u16>()
    }

    pub fn write_u32(&mut self, n: u32) -> usize {
        self.buf.extend_from_slice(&n.to_ne_bytes());

        size_of::<u32>()
    }

    pub fn write_i32(&mut self, n: i32) -> usize {
        self.buf.extend_from_slice(&n.to_ne_bytes());

        size_of::<i32>()
    }

    pub fn _write_u64(&mut self, n: u64) -> usize {
        self.buf.extend_from_slice(&n.to_ne_bytes());

        size_of::<u64>()
    }

    pub fn write_struct<T>(&mut self, val: T) -> usize {
        let bytes = unsafe {
            std::slice::from_raw_parts(&val as *const T as *const u8, std::mem::size_of::<T>())
        };
        self.write_bytes(bytes)
    }
}

pub struct Reader<'a> {
    buf: &'a [u8],
    cursor: usize,
}

#[allow(unused)]
impl Reader<'_> {
    fn check_bound(&self, n: usize) -> bool {
        (n + self.cursor) < self.buf.len()
    }

    fn _skip_to_align(&mut self) {
        let aligned = (self.cursor + 3) & !3;
        self.cursor = aligned.min(self.buf.len());
    }

    pub fn _read_bytes(&mut self, n: usize) -> Result<Vec<u8>, CoreError> {
        let mut val: Vec<u8> = Vec::new();

        if !self.check_bound(n) {
            return Err(NetworkError::ReadOutOfRange.into());
        }

        for i in 0..n {
            val.push(self.buf[i]);
            self.cursor += 1;
        }

        Ok(val)
    }

    pub fn _read_u8(&mut self) -> Result<u8, CoreError> {
        const BYTE_SIZE: usize = size_of::<u8>();

        if !self.check_bound(BYTE_SIZE) {
            return Err(NetworkError::ReadOutOfRange.into());
        }

        let val: [u8; BYTE_SIZE] = self.buf[self.cursor..self.cursor + BYTE_SIZE]
            .try_into()
            .map_err(|_| NetworkError::ReadFailure("u8".into()))?;

        self.cursor += BYTE_SIZE;
        Ok(u8::from_ne_bytes(val))
    }

    pub fn _read_u16(&mut self) -> Result<u16, CoreError> {
        const BYTE_SIZE: usize = size_of::<u16>();
        if !self.check_bound(BYTE_SIZE) {
            return Err(NetworkError::ReadOutOfRange.into());
        }
        let val: [u8; BYTE_SIZE] = self.buf[self.cursor..self.cursor + BYTE_SIZE]
            .try_into()
            .map_err(|_| NetworkError::ReadFailure("u16".into()))?;

        self.cursor += BYTE_SIZE;
        Ok(u16::from_ne_bytes(val))
    }

    pub fn _read_u32(&mut self) -> Result<u32, CoreError> {
        const BYTE_SIZE: usize = size_of::<u32>();
        if !self.check_bound(BYTE_SIZE) {
            return Err(NetworkError::ReadOutOfRange.into());
        }
        let val: [u8; BYTE_SIZE] = self.buf[self.cursor..self.cursor + BYTE_SIZE]
            .try_into()
            .map_err(|_| NetworkError::ReadFailure("u32".into()))?;

        self.cursor += BYTE_SIZE;
        Ok(u32::from_ne_bytes(val))
    }

    pub fn _read_u64(&mut self) -> Result<u64, CoreError> {
        const BYTE_SIZE: usize = size_of::<u64>();
        if !self.check_bound(BYTE_SIZE) {
            return Err(NetworkError::ReadOutOfRange.into());
        }
        let val: [u8; BYTE_SIZE] = self.buf[self.cursor..self.cursor + BYTE_SIZE]
            .try_into()
            .map_err(|_| NetworkError::ReadFailure("u64".into()))?;

        self.cursor += BYTE_SIZE;
        Ok(u64::from_ne_bytes(val))
    }
}

#[repr(C)]
struct Rtmsg {
    rtm_family: u8,
    rtm_dst_len: u8,
    rtm_src_len: u8,
    rtm_tos: u8,
    rtm_table: u8,
    rtm_protocol: u8,
    rtm_scope: u8,
    rtm_type: u8,
    rtm_flags: u32,
}

#[allow(clippy::too_many_arguments)]
fn rtmsg_builder(
    rtm_family: u8,
    rtm_dst_len: u8,
    rtm_src_len: u8,
    rtm_tos: u8,
    rtm_table: u8,
    rtm_protocol: u8,
    rtm_scope: u8,
    rtm_type: u8,
    rtm_flags: u32,
) -> Rtmsg {
    Rtmsg {
        rtm_family,
        rtm_dst_len,
        rtm_src_len,
        rtm_tos,
        rtm_table,
        rtm_protocol,
        rtm_scope,
        rtm_type,
        rtm_flags,
    }
}

fn nlmsghdr_builder(
    nlmsg_len: u32,
    nlmsg_type: u16,
    nlmsg_flags: u16,
    nlmsg_seq: u32,
    nlmsg_pid: u32,
) -> nlmsghdr {
    nlmsghdr {
        nlmsg_len,
        nlmsg_type,
        nlmsg_flags,
        nlmsg_seq,
        nlmsg_pid,
    }
}

fn ifinfomsg_builder(
    ifi_family: u8,
    ifi_type: u16,
    ifi_index: i32,
    ifi_flags: u32,
    ifi_change: u32,
) -> ifinfomsg {
    let mut iimsg: ifinfomsg = unsafe { std::mem::zeroed() };

    iimsg.ifi_family = ifi_family as c_uchar;
    iimsg.ifi_type = ifi_type as c_ushort;
    iimsg.ifi_index = ifi_index as c_int;
    iimsg.ifi_flags = ifi_flags as c_uint;
    iimsg.ifi_change = ifi_change as c_uint;
    iimsg
}

fn rtattr_builder(rta_len: u16, rta_type: u16) -> rtattr {
    rtattr {
        rta_len: rta_len as c_ushort,
        rta_type: rta_type as c_ushort,
    }
}

fn ifaddrmsg_builder(
    ifa_family: u8,
    ifa_prefixlen: u8,
    ifa_flags: u8,
    ifa_scope: u8,
    ifa_index: u32,
) -> ifaddrmsg {
    ifaddrmsg {
        ifa_family,
        ifa_prefixlen,
        ifa_flags,
        ifa_scope,
        ifa_index,
    }
}

fn recv_ack(socket: BorrowedFd) -> Result<(), CoreError> {
    let mut buf: [u8; 4096] = [0u8; 4096];

    let _ =
        recv(socket.as_raw_fd(), &mut buf, MsgFlags::empty()).map_err(NetworkError::RecvFailure)?;

    let nlmsg = unsafe { &*(buf.as_ptr() as *const nlmsghdr) };

    if nlmsg.nlmsg_type as i32 == libc::NLMSG_ERROR {
        let err = unsafe { &*(buf.as_ptr().add(size_of::<nlmsghdr>()) as *const libc::nlmsgerr) };
        if err.error != 0 {
            let e = format!(
                "netlink error: {}",
                std::io::Error::from_raw_os_error(-err.error)
            );
            return Err(NetworkError::NetlinkFailure(e).into());
        }
    }

    Ok(())
}

fn nest_start(w: &mut Writer, rta_type: u16) -> usize {
    let pos = w.pos();

    w.write_u16(0);
    w.write_u16(rta_type);
    pos
}

fn nest_end(w: &mut Writer, pos: usize) {
    let len = (w.pos() - pos) as u16;

    w.buf[pos..pos + 2].copy_from_slice(&len.to_ne_bytes());
    w.pad_to_align();
}

fn create_netlink_socket() -> Result<OwnedFd, CoreError> {
    let socket = socket(
        AddressFamily::Netlink,
        SockType::Raw,
        SockFlag::empty(),
        Some(SockProtocol::NetlinkRoute),
    )
    .map_err(NetworkError::CreateSocketFailure)?;

    let addr = NetlinkAddr::new(0, 0);
    bind(socket.as_raw_fd(), &addr).map_err(NetworkError::BindFailure)?;

    Ok(socket)
}

fn create_veth_pair(
    socket: BorrowedFd,
    w: &mut Writer,
    host: &str,
    peer: &str,
) -> Result<(), CoreError> {
    let kind = "veth";
    let info_kind_rta = rtattr_builder(
        ((2 * size_of::<u16>()) + kind.len()) as u16,
        libc::IFLA_INFO_KIND,
    );
    let host_name_rta = rtattr_builder(
        ((2 * size_of::<u16>()) + host.len() + 1) as u16,
        libc::IFLA_IFNAME,
    );
    let peer_name_rta = rtattr_builder(
        ((2 * size_of::<u16>()) + peer.len() + 1) as u16,
        libc::IFLA_IFNAME,
    );
    let v1_ifinfo = ifinfomsg_builder(libc::AF_UNSPEC as u8, 0, 0, 0, 0);
    let v1p_ifinfo: ifinfomsg = unsafe { std::mem::zeroed() };
    let nlmsg = nlmsghdr_builder(
        0,
        libc::RTM_NEWLINK,
        (libc::NLM_F_EXCL | libc::NLM_F_CREATE | libc::NLM_F_REQUEST | libc::NLM_F_ACK) as u16,
        0,
        0,
    );

    let _ = w.write_struct(nlmsg);
    let _ = w.write_struct(v1_ifinfo);
    let _ = w.write_struct(host_name_rta);
    let _ = w.write_bytes(host.as_bytes());
    let _ = w.write_u8(0u8);
    w.pad_to_align();
    let link_info_pos = nest_start(w, libc::IFLA_LINKINFO);
    let _ = w.write_struct(info_kind_rta);
    let _ = w.write_bytes(kind.as_bytes());
    w.pad_to_align();
    let info_data_pos = nest_start(w, libc::IFLA_INFO_DATA);
    let veth_info_pos = nest_start(w, VETH_INFO_PEER);
    let _ = w.write_struct(v1p_ifinfo);
    let _ = w.write_struct(peer_name_rta);
    let _ = w.write_bytes(peer.as_bytes());
    let _ = w.write_u8(0u8);
    w.pad_to_align();
    nest_end(w, veth_info_pos);
    nest_end(w, info_data_pos);
    nest_end(w, link_info_pos);
    let pos = w.pos() as u32;
    w.buf[0..4].copy_from_slice(&pos.to_ne_bytes());

    let _ = send(socket.as_raw_fd(), &w.buf, MsgFlags::MSG_WAITALL)
        .map_err(NetworkError::SendFailure)?;
    w.flush();

    recv_ack(socket.as_fd())?;

    Ok(())
}

fn set_interface_up(socket: BorrowedFd, w: &mut Writer, iface_id: u32) -> Result<(), CoreError> {
    let nlmsghdr = nlmsghdr_builder(
        0,
        libc::RTM_NEWLINK,
        (libc::NLM_F_REQUEST | libc::NLM_F_ACK) as u16,
        0,
        0,
    );
    let ifi = ifinfomsg_builder(
        libc::AF_UNSPEC as u8,
        0,
        iface_id as i32,
        libc::IFF_UP as u32,
        0x1u32,
    );

    let _ = w.write_struct(nlmsghdr);
    let _ = w.write_struct(ifi);
    let total_len = w.pos() as u32;
    w.buf[0..4].copy_from_slice(&total_len.to_ne_bytes());

    let _ = send(socket.as_raw_fd(), &w.buf, MsgFlags::MSG_WAITALL)
        .map_err(NetworkError::SendFailure)?;
    w.flush();

    recv_ack(socket.as_fd())?;

    Ok(())
}

fn set_ip_addr(
    socket: BorrowedFd,
    w: &mut Writer,
    iface_id: u32,
    ip: Ipv4Addr,
    prefix_len: u8,
) -> Result<(), CoreError> {
    let nlmsghdr = nlmsghdr_builder(
        0,
        libc::RTM_NEWADDR,
        (libc::NLM_F_REQUEST | libc::NLM_F_ACK | libc::NLM_F_EXCL | libc::NLM_F_CREATE) as u16,
        0,
        0,
    );
    let ifaddr = ifaddrmsg_builder(
        libc::AF_INET as u8,
        prefix_len, //as u8
        0,
        libc::RT_SCOPE_UNIVERSE,
        iface_id,
    );
    let local_attr = rtattr_builder(
        ((2 * size_of::<u16>()) + size_of::<u32>()) as u16,
        libc::IFA_LOCAL,
    );
    let address_attr = rtattr_builder(
        ((2 * size_of::<u16>()) + size_of::<u32>()) as u16,
        libc::IFA_ADDRESS,
    );
    let ip_bytes = ip.octets();

    let _ = w.write_struct(nlmsghdr);
    let _ = w.write_struct(ifaddr);
    let _ = w.write_struct(local_attr);
    let _ = w.write_bytes(&ip_bytes);
    let _ = w.write_struct(address_attr);
    let _ = w.write_bytes(&ip_bytes);
    let total_len = w.pos() as u32;
    w.buf[0..4].copy_from_slice(&total_len.to_ne_bytes());

    let _ = send(socket.as_raw_fd(), &w.buf, MsgFlags::MSG_WAITALL)
        .map_err(NetworkError::SendFailure)?;
    w.flush();

    recv_ack(socket.as_fd())?;

    Ok(())
}

fn move_to_netns(
    socket: BorrowedFd,
    w: &mut Writer,
    i_id: &u32,
    child_fd: &File,
) -> Result<(), CoreError> {
    let nlmsg = nlmsghdr_builder(
        0,
        libc::RTM_NEWLINK,
        (libc::NLM_F_REQUEST | libc::NLM_F_ACK) as u16,
        0,
        0,
    );
    let ifi = ifinfomsg_builder(libc::AF_UNSPEC as u8, 0, *i_id as i32, 0, 0);
    let netns_fd_attr = rtattr_builder(
        ((2 * size_of::<u16>()) + size_of::<i32>()) as u16,
        libc::IFLA_NET_NS_FD,
    );

    let _ = w.write_struct(nlmsg);
    let _ = w.write_struct(ifi);
    let _ = w.write_struct(netns_fd_attr);
    let _ = w.write_i32(child_fd.as_raw_fd());
    w.pad_to_align();
    let total_len = w.pos() as u32;
    w.buf[0..4].copy_from_slice(&total_len.to_ne_bytes());

    let _ = send(socket.as_raw_fd(), &w.buf, MsgFlags::MSG_WAITALL)
        .map_err(NetworkError::SendFailure)?;
    w.flush();

    recv_ack(socket.as_fd())?;

    Ok(())
}

fn get_interface_index(socket: BorrowedFd, w: &mut Writer, i_name: &str) -> Result<u32, CoreError> {
    let nlmsghdr = nlmsghdr_builder(0, libc::RTM_GETLINK, libc::NLM_F_REQUEST as u16, 0, 0);
    let ifi = ifinfomsg_builder(libc::AF_UNSPEC as u8, 0, 0, 0, 0);
    let ext_mask_attr = rtattr_builder(
        (2 * size_of::<u16>() + size_of::<u32>()) as u16,
        libc::IFLA_EXT_MASK,
    );
    let ext_mask_val = RTEXT_FILTER_VF | RTEXT_FILTER_SKIP_STATS;
    let ifname_attr = rtattr_builder(
        (2 * size_of::<u16>() + i_name.len() + 1) as u16,
        libc::IFLA_IFNAME,
    );

    let _ = w.write_struct(nlmsghdr);
    let _ = w.write_struct(ifi);
    let _ = w.write_struct(ext_mask_attr);
    let _ = w.write_u32(ext_mask_val);
    w.pad_to_align();
    let _ = w.write_struct(ifname_attr);
    let _ = w.write_bytes(i_name.as_bytes());
    let _ = w.write_u8(0u8);
    w.pad_to_align();
    let total_len = w.pos() as u32;
    w.buf[0..4].copy_from_slice(&total_len.to_ne_bytes());

    let _ = send(socket.as_raw_fd(), &w.buf, MsgFlags::MSG_WAITALL)
        .map_err(NetworkError::SendFailure)?;
    w.flush();

    let mut b: [u8; 4096] = [0u8; 4096];

    let _ =
        recv(socket.as_raw_fd(), &mut b, MsgFlags::empty()).map_err(NetworkError::RecvFailure)?;

    let nlmsg = unsafe { &*(b.as_ptr() as *const nlmsghdr) };

    if nlmsg.nlmsg_type as i32 == libc::NLMSG_ERROR {
        let err = unsafe { &*(b.as_ptr().add(size_of::<nlmsghdr>()) as *const libc::nlmsgerr) };
        if err.error != 0 {
            let e = format!(
                "netlink error: {}",
                std::io::Error::from_raw_os_error(-err.error)
            );
            return Err(NetworkError::NetlinkFailure(e).into());
        }
    }

    let ifinfo = unsafe { &*(b.as_ptr().add(size_of::<nlmsghdr>()) as *const ifinfomsg) };

    Ok(ifinfo.ifi_index as u32)
}

fn add_default_route(socket: BorrowedFd, w: &mut Writer, ip: Ipv4Addr) -> Result<(), CoreError> {
    let nlmsg = nlmsghdr_builder(
        0,
        libc::RTM_NEWROUTE,
        (libc::NLM_F_REQUEST | libc::NLM_F_ACK | libc::NLM_F_EXCL | libc::NLM_F_CREATE) as u16,
        0,
        0,
    );
    let rtm = rtmsg_builder(
        libc::AF_INET as u8,
        0,
        0,
        0,
        libc::RT_TABLE_MAIN,
        libc::RTPROT_BOOT,
        libc::RT_SCOPE_UNIVERSE,
        libc::RTN_UNICAST,
        0,
    );
    let rtattr = rtattr_builder(
        ((2 * size_of::<u16>()) + size_of::<u32>()) as u16,
        libc::RTA_GATEWAY,
    );
    let ip_bytes = ip.octets();

    let _ = w.write_struct(nlmsg);
    let _ = w.write_struct(rtm);
    let _ = w.write_struct(rtattr);
    let _ = w.write_bytes(&ip_bytes);
    w.pad_to_align();
    let total_len = w.pos() as u32;
    w.buf[0..4].copy_from_slice(&total_len.to_ne_bytes());

    let _ = send(socket.as_raw_fd(), &w.buf, MsgFlags::MSG_WAITALL)
        .map_err(NetworkError::SendFailure)?;
    w.flush();

    recv_ack(socket.as_fd())?;

    Ok(())
}

pub(crate) fn create_network(container_id: &str, child_pid: &Pid) -> Result<(), CoreError> {
    let host_ns_fd = File::open("/proc/self/ns/net").map_err(NetworkError::OpenFailure)?;
    let peer_ns_fd = File::open(format!("/proc/{}/ns/net", child_pid.as_raw()))
        .map_err(NetworkError::OpenFailure)?;
    let suffix = &container_id[..container_id.len().min(9)];
    let host = suffix;
    // TODO: IP addresses are hardcoded (10.0.0.1/10.0.0.2)
    // Multiple containers will conflict. Production requires dynamic IP allocation
    // per container, e.g. derived from container ID or managed pool.
    let host_address = Ipv4Addr::new(10, 0, 0, 1);
    let peer_address = Ipv4Addr::new(10, 0, 0, 2);
    let peer = format!("{}_peer", suffix);
    let mut w = Writer { buf: Vec::new() };

    let host_sk = create_netlink_socket()?;
    create_veth_pair(host_sk.as_fd(), &mut w, host, &peer)?;
    let host_i_id = get_interface_index(host_sk.as_fd(), &mut w, host)?;
    set_ip_addr(host_sk.as_fd(), &mut w, host_i_id, host_address, 24u8)?;
    set_interface_up(host_sk.as_fd(), &mut w, host_i_id)?;
    let child_i_id = get_interface_index(host_sk.as_fd(), &mut w, &peer)?;
    move_to_netns(host_sk.as_fd(), &mut w, &child_i_id, &peer_ns_fd)?;

    setns(peer_ns_fd.as_fd(), CloneFlags::CLONE_NEWNET)
        .map_err(|_| NamespaceError::FailedToEnterNamespace("peer_ns_fd".to_string()))?;

    let child_sk = create_netlink_socket()?;
    set_ip_addr(child_sk.as_fd(), &mut w, child_i_id, peer_address, 24u8)?;
    set_interface_up(child_sk.as_fd(), &mut w, child_i_id)?;
    set_interface_up(child_sk.as_fd(), &mut w, 1)?;
    add_default_route(child_sk.as_fd(), &mut w, host_address)?;

    drop(child_sk);

    setns(host_ns_fd.as_fd(), CloneFlags::CLONE_NEWNET)
        .map_err(|_| NamespaceError::FailedToEnterNamespace("host_ns_fd".to_string()))?;

    // TODO: replace with NETLINK_NETFILTER implementation
    // iptables NAT rule: masquerade container traffic through host interface
    // see: man 8 nft, include/uapi/linux/netfilter/nf_tables.h
    std::fs::write("/proc/sys/net/ipv4/ip_forward", "1").map_err(NetworkError::WriteFailure)?;
    std::process::Command::new("iptables")
        .args([
            "-t",
            "nat",
            "-A",
            "POSTROUTING",
            "-s",
            "10.0.0.0/24",
            "-o",
            "ens2",
            "-j",
            "MASQUERADE",
        ])
        .status()
        .map_err(NetworkError::CommandFailure)?;

    Ok(())
}
