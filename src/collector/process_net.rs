//! Best-effort per-process network byte counters.
//!
//! - **Linux:** map socket inodes from `/proc/<pid>/fd` and attribute TCP
//!   cumulative bytes from netlink `SOCK_DIAG` (`tcp_info` bytes received /
//!   acked). Processes in a **private network namespace** fall back to
//!   `/proc/<pid>/net/dev` when sock-diag yields nothing for that PID.
//! - **Other OS:** empty map — callers keep `net_rx_bytes` / `net_tx_bytes` at 0.
//!
//! Failures are soft: missing `/proc` entries, EPERM, or netlink errors skip
//! that PID rather than failing the whole sample.

use std::collections::HashMap;

/// Cumulative network bytes attributed to one process.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessNetBytes {
    pub rx: u64,
    pub tx: u64,
}

impl ProcessNetBytes {
    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            rx: self.rx.saturating_add(other.rx),
            tx: self.tx.saturating_add(other.tx),
        }
    }
}

/// Best-effort sample of cumulative network bytes for the given PIDs.
///
/// Returns only PIDs for which at least one source reported data. Missing
/// keys mean "unknown / unsupported" — use `0` counters.
pub fn sample_process_net(pids: &[u32]) -> HashMap<u32, ProcessNetBytes> {
    #[cfg(target_os = "linux")]
    {
        linux::sample(pids)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pids;
        HashMap::new()
    }
}

/// Whether this build can attempt per-process network counters.
pub fn process_net_supported() -> bool {
    cfg!(target_os = "linux")
}

/// Parse a `/proc/<pid>/fd/N` symlink target for a socket inode.
///
/// Accepts `socket:[12345]` (and trims whitespace). Returns `None` for non-sockets.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_socket_fd_target(target: &str) -> Option<u64> {
    let t = target.trim();
    let rest = t.strip_prefix("socket:[")?.strip_suffix(']')?;
    rest.parse().ok()
}

/// Parse `/proc/net/dev` body (with or without the two header lines) into total
/// receive/transmit bytes across all interfaces.
///
/// Lines that fail to parse are skipped. Interface names are ignored; loopback
/// is **included** so private-netns totals match kernel accounting.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_proc_net_dev(content: &str) -> ProcessNetBytes {
    let mut rx = 0u64;
    let mut tx = 0u64;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Inter-") || line.starts_with(" face") {
            continue;
        }
        // Format: iface: rx_bytes rx_packets ... tx_bytes tx_packets ...
        let Some(colon) = line.find(':') else {
            continue;
        };
        let fields: Vec<&str> = line[colon + 1..].split_whitespace().collect();
        // Need at least: rx_bytes, rx_packets, rx_errs, rx_drop, rx_fifo, rx_frame,
        // rx_compressed, rx_multicast, tx_bytes (9th field, index 8).
        if fields.len() < 9 {
            continue;
        }
        let Ok(r) = fields[0].parse::<u64>() else {
            continue;
        };
        let Ok(t) = fields[8].parse::<u64>() else {
            continue;
        };
        rx = rx.saturating_add(r);
        tx = tx.saturating_add(t);
    }

    ProcessNetBytes { rx, tx }
}

/// Aggregate socket-level byte counters onto PIDs via inode ownership.
///
/// Each socket's bytes are added to **every** PID that holds that inode
/// (shared sockets appear on each holder — acceptable for per-process views).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn aggregate_bytes_by_pid(
    pid_to_inodes: &HashMap<u32, Vec<u64>>,
    inode_to_bytes: &HashMap<u64, ProcessNetBytes>,
) -> HashMap<u32, ProcessNetBytes> {
    let mut out: HashMap<u32, ProcessNetBytes> = HashMap::new();

    for (pid, inodes) in pid_to_inodes {
        let mut total = ProcessNetBytes::default();
        let mut any = false;
        for inode in inodes {
            if let Some(b) = inode_to_bytes.get(inode) {
                total = total.saturating_add(*b);
                any = true;
            }
        }
        if any {
            out.insert(*pid, total);
        }
    }

    out
}

/// Parse the inode column from a single `/proc/net/{tcp,tcp6,udp,udp6}` data line.
///
/// Header lines and malformed rows return `None`. Inode is field index 9
/// (0-based) after splitting on whitespace (standard proc net table layout).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_proc_net_table_inode(line: &str) -> Option<u64> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("sl") || line.starts_with("Num") {
        return None;
    }
    let fields: Vec<&str> = line.split_whitespace().collect();
    // sl local_address rem_address st tx_queue:rx_queue tr:tm->when retrnsmt uid timeout inode
    if fields.len() < 10 {
        return None;
    }
    fields[9].parse().ok().filter(|&ino| ino != 0)
}

/// Collect unique non-zero inodes from a full `/proc/net/*` table body.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_proc_net_table_inodes(content: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for line in content.lines() {
        if let Some(ino) = parse_proc_net_table_inode(line) {
            out.push(ino);
        }
    }
    out
}

/// Extract `tcpi_bytes_acked` (tx) and `tcpi_bytes_received` (rx) from a
/// `struct tcp_info` byte blob using documented field offsets (`linux/tcp.h`).
///
/// Offsets assume the standard layout through `tcpi_bytes_received` (Linux ~4.2+):
/// - `tcpi_bytes_acked` at byte 120
/// - `tcpi_bytes_received` at byte 128
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_tcp_info_bytes(data: &[u8]) -> Option<ProcessNetBytes> {
    const OFF_BYTES_ACKED: usize = 120;
    const OFF_BYTES_RECEIVED: usize = 128;
    if data.len() < OFF_BYTES_RECEIVED + 8 {
        return None;
    }
    let tx = u64::from_ne_bytes(data[OFF_BYTES_ACKED..OFF_BYTES_ACKED + 8].try_into().ok()?);
    let rx = u64::from_ne_bytes(
        data[OFF_BYTES_RECEIVED..OFF_BYTES_RECEIVED + 8]
            .try_into()
            .ok()?,
    );
    Some(ProcessNetBytes { rx, tx })
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{
        aggregate_bytes_by_pid, parse_proc_net_dev, parse_socket_fd_target, parse_tcp_info_bytes,
        ProcessNetBytes,
    };
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::io::FromRawFd;
    use std::path::PathBuf;

    pub(super) fn sample(pids: &[u32]) -> HashMap<u32, ProcessNetBytes> {
        if pids.is_empty() {
            return HashMap::new();
        }

        let pid_to_inodes = collect_socket_inodes(pids);
        let inode_bytes = sock_diag_tcp_bytes().unwrap_or_default();
        let mut by_pid = aggregate_bytes_by_pid(&pid_to_inodes, &inode_bytes);

        // Private-netns fallback for PIDs with no sock-diag attribution.
        let self_netns = read_netns_id(None);
        for &pid in pids {
            if by_pid.contains_key(&pid) {
                continue;
            }
            let Some(netns) = read_netns_id(Some(pid)) else {
                continue;
            };
            if self_netns.as_ref() == Some(&netns) {
                // Shared with this process's netns: /proc/pid/net/dev is host-wide.
                continue;
            }
            if let Some(bytes) = read_pid_net_dev(pid) {
                if bytes.rx > 0 || bytes.tx > 0 {
                    by_pid.insert(pid, bytes);
                }
            }
        }

        by_pid
    }

    fn collect_socket_inodes(pids: &[u32]) -> HashMap<u32, Vec<u64>> {
        let mut map = HashMap::new();
        for &pid in pids {
            let dir = PathBuf::from(format!("/proc/{pid}/fd"));
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            let mut inodes = HashSet::new();
            for ent in entries.flatten() {
                let path = ent.path();
                let Ok(target) = fs::read_link(&path) else {
                    continue;
                };
                let target = target.to_string_lossy();
                if let Some(ino) = parse_socket_fd_target(&target) {
                    inodes.insert(ino);
                }
            }
            if !inodes.is_empty() {
                map.insert(pid, inodes.into_iter().collect());
            }
        }
        map
    }

    fn read_netns_id(pid: Option<u32>) -> Option<String> {
        let path = match pid {
            Some(p) => format!("/proc/{p}/ns/net"),
            None => "/proc/self/ns/net".to_string(),
        };
        fs::read_link(path)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }

    fn read_pid_net_dev(pid: u32) -> Option<ProcessNetBytes> {
        let content = fs::read_to_string(format!("/proc/{pid}/net/dev")).ok()?;
        Some(parse_proc_net_dev(&content))
    }

    // --- Minimal SOCK_DIAG (INET) dump for TCP_INFO byte counters ------------

    const AF_NETLINK: i32 = 16;
    const AF_INET: u8 = 2;
    const AF_INET6: u8 = 10;
    const NETLINK_SOCK_DIAG: i32 = 4;
    const SOCK_DIAG_BY_FAMILY: u16 = 20;
    const NLM_F_REQUEST: u16 = 0x01;
    const NLM_F_DUMP: u16 = 0x300;
    const NLMSG_DONE: u16 = 0x3;
    const NLMSG_ERROR: u16 = 0x2;
    const IPPROTO_TCP: u8 = 6;
    /// (1 << (INET_DIAG_INFO - 1)); INET_DIAG_INFO = 2
    const INET_DIAG_INFO_EXT: u8 = 1 << (2 - 1);
    /// All TCP states bitmask used by `ss` / iproute2 dumps.
    const TCPF_ALL: u32 = 0xFFF;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NlMsgHdr {
        nlmsg_len: u32,
        nlmsg_type: u16,
        nlmsg_flags: u16,
        nlmsg_seq: u32,
        nlmsg_pid: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct InetDiagSockId {
        idiag_sport: u16,
        idiag_dport: u16,
        idiag_src: [u32; 4],
        idiag_dst: [u32; 4],
        idiag_if: u32,
        idiag_cookie: [u32; 2],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct InetDiagReqV2 {
        sdiag_family: u8,
        sdiag_protocol: u8,
        idiag_ext: u8,
        pad: u8,
        idiag_states: u32,
        id: InetDiagSockId,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct InetDiagMsg {
        idiag_family: u8,
        idiag_state: u8,
        idiag_timer: u8,
        idiag_retrans: u8,
        id: InetDiagSockId,
        idiag_expires: u32,
        idiag_rqueue: u32,
        idiag_wqueue: u32,
        idiag_uid: u32,
        idiag_inode: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RtAttr {
        rta_len: u16,
        rta_type: u16,
    }

    fn sock_diag_tcp_bytes() -> Option<HashMap<u64, ProcessNetBytes>> {
        let mut map = HashMap::new();
        dump_family(AF_INET, &mut map)?;
        // IPv6 dump is best-effort; keep IPv4 results if it fails.
        let _ = dump_family(AF_INET6, &mut map);
        Some(map)
    }

    fn dump_family(family: u8, out: &mut HashMap<u64, ProcessNetBytes>) -> Option<()> {
        let fd = unsafe { libc_socket(AF_NETLINK, libc_sock_dgram(), NETLINK_SOCK_DIAG) };
        if fd < 0 {
            return None;
        }
        let mut sock = unsafe { std::fs::File::from_raw_fd(fd) };

        let req = InetDiagReqV2 {
            sdiag_family: family,
            sdiag_protocol: IPPROTO_TCP,
            idiag_ext: INET_DIAG_INFO_EXT,
            pad: 0,
            idiag_states: TCPF_ALL,
            id: InetDiagSockId {
                idiag_sport: 0,
                idiag_dport: 0,
                idiag_src: [0; 4],
                idiag_dst: [0; 4],
                idiag_if: 0,
                idiag_cookie: [0; 2],
            },
        };

        let hdr_len = std::mem::size_of::<NlMsgHdr>();
        let req_len = std::mem::size_of::<InetDiagReqV2>();
        let total = nlmsg_align(hdr_len + req_len);

        let mut buf = vec![0u8; total];
        let hdr = NlMsgHdr {
            nlmsg_len: (hdr_len + req_len) as u32,
            nlmsg_type: SOCK_DIAG_BY_FAMILY,
            nlmsg_flags: NLM_F_REQUEST | NLM_F_DUMP,
            nlmsg_seq: 1,
            nlmsg_pid: 0,
        };
        // SAFETY: buf sized for header + request; types are #[repr(C)].
        unsafe {
            std::ptr::write_unaligned(buf.as_mut_ptr() as *mut NlMsgHdr, hdr);
            std::ptr::write_unaligned(buf.as_mut_ptr().add(hdr_len) as *mut InetDiagReqV2, req);
        }

        if sock.write_all(&buf).is_err() {
            return None;
        }

        let mut rbuf = vec![0u8; 65536];
        loop {
            let n = match sock.read(&mut rbuf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => return None,
            };
            let mut offset = 0usize;
            while offset + hdr_len <= n {
                let nh = unsafe {
                    std::ptr::read_unaligned(rbuf.as_ptr().add(offset) as *const NlMsgHdr)
                };
                let len = nh.nlmsg_len as usize;
                if len < hdr_len || offset + len > n {
                    break;
                }
                match nh.nlmsg_type {
                    NLMSG_DONE => return Some(()),
                    NLMSG_ERROR => return None,
                    SOCK_DIAG_BY_FAMILY => {
                        parse_diag_message(&rbuf[offset + hdr_len..offset + len], out);
                    }
                    _ => {}
                }
                offset += nlmsg_align(len);
            }
        }
        Some(())
    }

    fn parse_diag_message(payload: &[u8], out: &mut HashMap<u64, ProcessNetBytes>) {
        let msg_size = std::mem::size_of::<InetDiagMsg>();
        if payload.len() < msg_size {
            return;
        }
        let msg = unsafe { std::ptr::read_unaligned(payload.as_ptr() as *const InetDiagMsg) };
        let inode = msg.idiag_inode as u64;
        if inode == 0 {
            return;
        }

        // Attributes follow the inet_diag_msg, aligned.
        let mut attr_off = nlmsg_align(msg_size);
        let mut bytes = ProcessNetBytes::default();
        let mut found_info = false;

        while attr_off + std::mem::size_of::<RtAttr>() <= payload.len() {
            let rta = unsafe {
                std::ptr::read_unaligned(payload.as_ptr().add(attr_off) as *const RtAttr)
            };
            let rta_len = rta.rta_len as usize;
            if rta_len < std::mem::size_of::<RtAttr>() || attr_off + rta_len > payload.len() {
                break;
            }
            // INET_DIAG_INFO = 2
            if rta.rta_type == 2 {
                let data_off = attr_off + std::mem::size_of::<RtAttr>();
                let data_len = rta_len - std::mem::size_of::<RtAttr>();
                if let Some(b) = parse_tcp_info_bytes(&payload[data_off..data_off + data_len]) {
                    bytes = b;
                    found_info = true;
                }
            }
            attr_off += rta_align(rta_len);
        }

        if found_info {
            out.entry(inode)
                .and_modify(|e| *e = e.saturating_add(bytes))
                .or_insert(bytes);
        }
    }

    fn nlmsg_align(len: usize) -> usize {
        (len + 3) & !3
    }

    fn rta_align(len: usize) -> usize {
        (len + 3) & !3
    }

    // Thin libc shims so we do not pull the full `libc` crate on non-Linux.
    // On Linux these match the platform ABI.

    fn libc_sock_dgram() -> i32 {
        2 // SOCK_DGRAM
    }

    unsafe fn libc_socket(domain: i32, ty: i32, protocol: i32) -> i32 {
        extern "C" {
            fn socket(domain: i32, ty: i32, protocol: i32) -> i32;
        }
        socket(domain, ty, protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parse_socket_fd_target_accepts_standard_form() {
        assert_eq!(parse_socket_fd_target("socket:[12345]"), Some(12345));
        assert_eq!(parse_socket_fd_target("  socket:[9]  "), Some(9));
    }

    #[test]
    fn parse_socket_fd_target_rejects_non_sockets() {
        assert_eq!(parse_socket_fd_target("/dev/null"), None);
        assert_eq!(parse_socket_fd_target("pipe:[1]"), None);
        assert_eq!(parse_socket_fd_target("socket:[abc]"), None);
        assert_eq!(parse_socket_fd_target("socket:[1"), None);
        assert_eq!(parse_socket_fd_target(""), None);
    }

    #[test]
    fn parse_proc_net_dev_sums_interfaces() {
        let body = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1000       10    0    0    0     0          0         0     1000       10    0    0    0     0       0          0
  eth0: 2000       20    0    0    0     0          0         0     4000       20    0    0    0     0       0          0
";
        let b = parse_proc_net_dev(body);
        assert_eq!(b.rx, 3000);
        assert_eq!(b.tx, 5000);
    }

    #[test]
    fn parse_proc_net_dev_skips_garbage_and_short_lines() {
        let body = "\
face: 1 2 3
eth0: not_a_number 0 0 0 0 0 0 0 9 0 0 0 0 0 0 0
eth1: 10 0 0 0 0 0 0 0 20 0 0 0 0 0 0 0
";
        let b = parse_proc_net_dev(body);
        assert_eq!(b.rx, 10);
        assert_eq!(b.tx, 20);
    }

    #[test]
    fn parse_proc_net_dev_empty_is_zero() {
        assert_eq!(parse_proc_net_dev(""), ProcessNetBytes::default());
        assert_eq!(
            parse_proc_net_dev("Inter-| foo\n face | bar\n"),
            ProcessNetBytes::default()
        );
    }

    #[test]
    fn parse_proc_net_table_inode_reads_column() {
        // Realistic-ish tcp line (fields abbreviated but index 9 = inode).
        let line = "   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 123456 1 0000000000000000 100 0 0 10 0";
        assert_eq!(parse_proc_net_table_inode(line), Some(123456));
    }

    #[test]
    fn parse_proc_net_table_inode_skips_header_and_zero() {
        assert_eq!(
            parse_proc_net_table_inode(
                "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode"
            ),
            None
        );
        let line = "   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 0 1 0000000000000000 100 0 0 10 0";
        assert_eq!(parse_proc_net_table_inode(line), None);
    }

    #[test]
    fn aggregate_bytes_by_pid_sums_owned_sockets() {
        let mut pid_inodes = HashMap::new();
        pid_inodes.insert(1, vec![10, 11]);
        pid_inodes.insert(2, vec![11]); // shared inode 11
        pid_inodes.insert(3, vec![99]); // no bytes

        let mut inode_bytes = HashMap::new();
        inode_bytes.insert(10, ProcessNetBytes { rx: 100, tx: 50 });
        inode_bytes.insert(11, ProcessNetBytes { rx: 5, tx: 7 });

        let out = aggregate_bytes_by_pid(&pid_inodes, &inode_bytes);
        assert_eq!(out.get(&1), Some(&ProcessNetBytes { rx: 105, tx: 57 }));
        assert_eq!(out.get(&2), Some(&ProcessNetBytes { rx: 5, tx: 7 }));
        assert!(!out.contains_key(&3));
    }

    #[test]
    fn aggregate_empty_inputs() {
        let out = aggregate_bytes_by_pid(&HashMap::new(), &HashMap::new());
        assert!(out.is_empty());
    }

    #[test]
    fn process_net_bytes_saturating_add() {
        let a = ProcessNetBytes {
            rx: u64::MAX,
            tx: 1,
        };
        let b = ProcessNetBytes { rx: 10, tx: 2 };
        let s = a.saturating_add(b);
        assert_eq!(s.rx, u64::MAX);
        assert_eq!(s.tx, 3);
    }

    #[test]
    fn sample_process_net_on_unsupported_os_is_empty() {
        // On macOS/Windows CI this is the contract: no keys ⇒ callers keep 0.
        if !process_net_supported() {
            let map = sample_process_net(&[1, 2, 3]);
            assert!(map.is_empty());
        }
    }

    #[test]
    fn process_net_supported_matches_cfg() {
        assert_eq!(process_net_supported(), cfg!(target_os = "linux"));
    }

    #[test]
    fn parse_tcp_info_bytes_reads_acked_and_received() {
        let mut data = vec![0u8; 136];
        // tx = bytes_acked @ 120
        data[120..128].copy_from_slice(&1000u64.to_ne_bytes());
        // rx = bytes_received @ 128
        data[128..136].copy_from_slice(&2000u64.to_ne_bytes());
        let b = parse_tcp_info_bytes(&data).expect("full tcp_info");
        assert_eq!(b.tx, 1000);
        assert_eq!(b.rx, 2000);
    }

    #[test]
    fn parse_tcp_info_bytes_rejects_short_buffer() {
        assert!(parse_tcp_info_bytes(&[0u8; 100]).is_none());
        assert!(parse_tcp_info_bytes(&[]).is_none());
    }

    #[test]
    fn parse_proc_net_table_inodes_collects_nonzero() {
        let table = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 111 1 0000000000000000 100 0 0 10 0
   1: 00000000:0017 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 0 1 0000000000000000 100 0 0 10 0
   2: 0100007F:0035 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 222 1 0000000000000000 100 0 0 10 0
";
        let inodes = parse_proc_net_table_inodes(table);
        assert_eq!(inodes, vec![111, 222]);
    }
}
