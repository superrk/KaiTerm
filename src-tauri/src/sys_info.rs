use crate::ssh_session::SessionManager;
use crate::error::DshellError;
use russh::ChannelMsg;
use tokio::time::{timeout, Duration};
use serde::Serialize;

// ── Data structures ──────────────────────────────────────────

#[derive(Serialize, Default)]
pub struct SysInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_freq_mhz: f64,
    pub cpu_max_mhz: f64,
    pub cpu_usage_pct: f64,
    pub memory: MemInfo,
    pub swap: MemInfo,
    pub disks: Vec<DiskInfo>,
    pub interfaces: Vec<NetInfo>,
    pub processes: Vec<ProcInfo>,
    pub ports: Vec<PortInfo>,
}

#[derive(Serialize, Default)]
pub struct MemInfo {
    pub total_mb: f64,
    pub used_mb: f64,
    pub pct: f64,
}

#[derive(Serialize, Default)]
pub struct DiskInfo {
    pub filesystem: String,
    pub mount: String,
    pub total: String,
    pub used: String,
    pub avail: String,
    pub pct: f64,
}

#[derive(Serialize, Default)]
pub struct NetInfo {
    pub name: String,
    pub ip: String,
    pub mac: String,
}

#[derive(Serialize, Default)]
pub struct ProcInfo {
    pub pid: u32,
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub command: String,
}

#[derive(Serialize, Default)]
pub struct PortInfo {
    pub port: u16,
    pub protocol: String,
    pub process: String,
}

// ── Tauri command ────────────────────────────────────────────

#[tauri::command]
pub async fn sysinfo_get(
    session_id: String,
    state: tauri::State<'_, SessionManager>,
) -> Result<SysInfo, DshellError> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .iter()
        .find(|s| s.id == session_id)
        .ok_or(DshellError::SessionNotFound(session_id.clone()))?;
    let handle = session.handle.clone();
    drop(sessions);

    let script = r#"
echo "===UNAME==="; uname -a
echo "===OS==="; (cat /etc/os-release 2>/dev/null | grep -E '^PRETTY_NAME|^NAME|^VERSION_ID' | head -3 || cat /etc/*release 2>/dev/null | head -5)
echo "===UPTIME==="; cat /proc/uptime 2>/dev/null
echo "===CPUINFO==="; lscpu 2>/dev/null | grep -E 'Model name|CPU\(s\)|Thread|Core|Socket'
echo "===CPUSTAT==="; head -1 /proc/stat 2>/dev/null
echo "===MEM==="; free -m 2>/dev/null
echo "===DISK==="; df -h --exclude-type=tmpfs --exclude-type=devtmpfs --exclude-type=overlay 2>/dev/null || df -h 2>/dev/null
echo "===NET==="; ip -o addr show 2>/dev/null
echo "===PROC==="; ps aux --sort=-%cpu 2>/dev/null | head -30
echo "===PORT==="; ss -tlnp 2>/dev/null | head -50
echo "===DONE==="
"#;

    let h = handle.lock().await;
    let mut channel = h
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {}", e))?;
    drop(h);

    let wrapped = format!("sh -c '{}'", script.replace('\'', "'\\''"));
    channel
        .exec(true, wrapped.as_str())
        .await
        .map_err(|e| format!("执行命令失败: {}", e))?;

    let mut raw = String::new();
    let collected = timeout(Duration::from_secs(30), async {
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    raw.push_str(&String::from_utf8_lossy(&data));
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
    })
    .await;

    // 远端命令卡死（如 ps/ss hang）时 timeout 触发，避免前端永久等待。
    if collected.is_err() {
        let _ = channel.close().await;
        return Err(DshellError::Msg("获取系统信息超时（30s），远端命令可能已卡死".to_string()));
    }
    let _ = channel.close().await;

    Ok(parse_all(&raw))
}

// ── Parser ───────────────────────────────────────────────────

fn parse_all(raw: &str) -> SysInfo {
    let mut info = SysInfo::default();

    // Split by section markers
    let sections: Vec<&str> = raw.split("===UNAME===").collect();
    if sections.len() > 1 {
        if let Some(body) = sections.get(1) {
            let rest = *body;
            // extract until next marker or end
            let text = take_until_marker(rest);
            parse_uname(&text, &mut info);
        }
    }

    if let Some(text) = extract_section(raw, "===OS===") {
        parse_os(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===CPUINFO===") {
        parse_cpuinfo(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===CPUSTAT===") {
        parse_cpustat(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===MEM===") {
        parse_mem(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===DISK===") {
        parse_disk(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===NET===") {
        parse_net(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===PROC===") {
        parse_proc(&text, &mut info);
    }
    if let Some(text) = extract_section(raw, "===PORT===") {
        parse_port(&text, &mut info);
    }

    info
}

fn extract_section<'a>(raw: &'a str, marker: &str) -> Option<&'a str> {
    let parts: Vec<&str> = raw.split(marker).collect();
    if parts.len() > 1 {
        Some(take_until_marker(parts[1]))
    } else {
        None
    }
}

fn take_until_marker(text: &str) -> &str {
    if let Some(pos) = text.find("===") {
        &text[..pos]
    } else {
        text
    }
}

fn parse_uname(text: &str, info: &mut SysInfo) {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    // Linux hostname 5.15.0 #1 SMP ...
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() >= 2 {
        info.hostname = parts[1].to_string();
    }
    if parts.len() >= 3 {
        info.kernel = parts[2].to_string();
    }
}

fn parse_os(text: &str, info: &mut SysInfo) {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("PRETTY_NAME=") || line.starts_with("NAME=") {
            let val = line.splitn(2, '=').nth(1).unwrap_or("").trim_matches('"');
            if info.os.is_empty() { info.os = val.to_string(); }
        }
        if line.starts_with("VERSION_ID=") {
            let val = line.splitn(2, '=').nth(1).unwrap_or("").trim_matches('"');
            if !info.os.contains(val) { info.os.push_str(&format!(" ({})", val)); }
        }
    }
    if info.os.is_empty() {
        info.os = text.lines().next().unwrap_or("").trim().to_string();
    }
}

fn parse_cpuinfo(text: &str, info: &mut SysInfo) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let (key, val) = if let Some((k, v)) = line.split_once('：') {
            (k.trim(), v.trim())
        } else if let Some((k, v)) = line.split_once(':') {
            (k.trim(), v.trim())
        } else {
            continue;
        };
        match key {
            "Model name" | "型号名称" => info.cpu_model = val.to_string(),
            "CPU(s)" => {
                if let Ok(n) = val.parse::<u32>() { info.cpu_cores = n; }
            }
            "CPU MHz" | "CPU 频率" | "CPU 当前频率" => {
                if let Ok(f) = val.parse::<f64>() { info.cpu_freq_mhz = f; }
            }
            "CPU max MHz" | "CPU 最大频率" => {
                if let Ok(f) = val.parse::<f64>() { info.cpu_max_mhz = f; }
            }
            _ => {}
        }
    }
}

fn parse_cpustat(text: &str, info: &mut SysInfo) {
    // /proc/stat first line: cpu  user nice system idle iowait irq softirq steal
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let vals: Vec<&str> = line.split_whitespace().collect();
    if vals.len() >= 5 {
        let total: u64 = vals[1..].iter().filter_map(|v| v.parse::<u64>().ok()).sum();
        let idle: u64 = vals[4].parse().unwrap_or(0);
        if total > 0 {
            info.cpu_usage_pct = ((total - idle) as f64 / total as f64) * 100.0;
        }
    }
}

fn parse_mem(text: &str, info: &mut SysInfo) {
    // free -m output:
    //               total  used  free  shared  buff/cache  available
    // Mem:          15984  8234  1234   567     6123        4567
    // Swap:         2048   123   1925
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("Mem:") || line.starts_with("mem:") {
            let vals: Vec<f64> = line.split_whitespace()
                .skip(1).filter_map(|v| v.parse::<f64>().ok()).collect();
            if vals.len() >= 3 {
                info.memory.total_mb = vals[0];
                info.memory.used_mb = vals[1];
                info.memory.pct = if vals[0] > 0.0 { vals[1] / vals[0] * 100.0 } else { 0.0 };
            }
        }
        if line.starts_with("Swap:") || line.starts_with("swap:") {
            let vals: Vec<f64> = line.split_whitespace()
                .skip(1).filter_map(|v| v.parse::<f64>().ok()).collect();
            if vals.len() >= 2 {
                info.swap.total_mb = vals[0];
                info.swap.used_mb = vals[1];
                info.swap.pct = if vals[0] > 0.0 { vals[1] / vals[0] * 100.0 } else { 0.0 };
            }
        }
    }
}

fn parse_disk(text: &str, info: &mut SysInfo) {
    // df -h output:
    // Filesystem      Size  Used Avail Use% Mounted on
    // /dev/sda1       100G   45G   55G  45% /
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let pct_str = parts[4].trim_end_matches('%');
            if let Ok(pct) = pct_str.parse::<f64>() {
                info.disks.push(DiskInfo {
                    filesystem: parts[0].to_string(),
                    total: parts[1].to_string(),
                    used: parts[2].to_string(),
                    avail: parts[3].to_string(),
                    pct,
                    mount: parts[5..].join(" "),
                });
            }
        }
    }
}

fn parse_net(text: &str, info: &mut SysInfo) {
    // ip -o addr show:
    // 1: lo    inet 127.0.0.1/8 ...
    // 1: lo    inet6 ::1/128 ...
    // 2: eth0  inet 192.168.1.100/24 ...
    // 2: eth0  link/ether 00:11:22:33:44:55 ...
    let mut current: Option<NetInfo> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }

        let ifname = parts[1].trim_end_matches(':');
        let family = parts[2];

        if family == "inet" && parts.len() >= 4 {
            let ip = parts[3].split('/').next().unwrap_or("").to_string();
            if ifname != "lo" {
                if let Some(ref mut n) = current {
                    if n.name == ifname { n.ip = ip; continue; }
                }
                let mut n = NetInfo::default();
                n.name = ifname.to_string();
                n.ip = ip;
                current = Some(n);
            }
        } else if family == "link/ether" && parts.len() >= 4 {
            let mac = parts[3].to_string();
            if ifname != "lo" {
                if current.as_ref().map_or(false, |n| n.name == ifname) {
                    if let Some(ref mut n) = current { n.mac = mac; }
                } else {
                    let mut n = NetInfo::default();
                    n.name = ifname.to_string();
                    n.mac = mac;
                    current = Some(n);
                }
            }
        }
    }
    if let Some(n) = current { info.interfaces.push(n); }
}

fn parse_proc(text: &str, info: &mut SysInfo) {
    // ps aux --sort=-%cpu
    // USER         PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND
    // root           1  0.0  0.1 123456 12345 ?        Ss   Jan01   0:01 /sbin/init
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 11 { continue; }
        if let Ok(pid) = parts[1].parse::<u32>() {
            let cpu: f64 = parts[2].parse().unwrap_or(0.0);
            let mem: f64 = parts[3].parse().unwrap_or(0.0);
            let cmd = parts[10..].join(" ");
            info.processes.push(ProcInfo { pid, cpu_pct: cpu, mem_pct: mem, command: cmd });
        }
    }
}

fn parse_port(text: &str, info: &mut SysInfo) {
    // ss -tlnp
    // State    Recv-Q   Send-Q   Local Address:Port   Peer Address:Port   Process
    // LISTEN   0        128      0.0.0.0:22           0.0.0.0:*           users:(("sshd",pid=1234,fd=3))
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }

        let addr_port = parts[3];
        let addr_parts: Vec<&str> = addr_port.rsplitn(2, ':').collect();
        let port = addr_parts[0].parse::<u16>().unwrap_or(0);
        if port == 0 { continue; }

        let protocol = if addr_port.starts_with('[') { "tcp6" } else { "tcp4" };

        let process = if parts.len() >= 6 {
            let proc_raw = parts[5];
            // Extract process name from users:(("sshd",pid=1234,...))
            if let Some(start) = proc_raw.find('"') {
                let rest = &proc_raw[start + 1..];
                if let Some(end) = rest.find('"') {
                    rest[..end].to_string()
                } else { String::new() }
            } else { String::new() }
        } else { String::new() };

        info.ports.push(PortInfo { port, protocol: protocol.to_string(), process });
    }
}

