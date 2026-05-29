use serde::{Deserialize, Serialize};
use serialport::SerialPortInfo;

/// 串口信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    /// 串口名称 (如 "COM3")
    pub name: String,
    /// 串口类型 (如 "CH340", "FTDI", "CP210x")
    pub port_type: String,
}

impl From<&SerialPortInfo> for PortInfo {
    fn from(info: &SerialPortInfo) -> Self {
        Self {
            name: info.port_name.clone(),
            port_type: guess_port_type(&info.port_name),
        }
    }
}

/// 根据端口名称猜测串口芯片类型
fn guess_port_type(name: &str) -> String {
    let upper = name.to_uppercase();
    if upper.contains("CH340") {
        "CH340".to_string()
    } else if upper.contains("FTDI") || upper.contains("FT232") {
        "FTDI".to_string()
    } else if upper.contains("CP210") || upper.contains("SILICON") {
        "CP210x".to_string()
    } else if upper.contains("PL2303") {
        "PL2303".to_string()
    } else if upper.contains("PROLIFIC") {
        "PL2303".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// 列出所有可用串口
pub fn list_ports() -> Vec<PortInfo> {
    serialport::available_ports()
        .unwrap_or_default()
        .iter()
        .map(PortInfo::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_ports_returns_vec() {
        let ports = list_ports();
        // 不应panic，无论是否有串口
        assert!(ports.is_empty() || ports.len() > 0);
    }

    #[test]
    fn test_guess_port_type_ch340() {
        assert_eq!(guess_port_type("COM3 (CH340)"), "CH340");
        assert_eq!(guess_port_type("ch340"), "CH340");
    }

    #[test]
    fn test_guess_port_type_ftdi() {
        assert_eq!(guess_port_type("COM4 (FT232R)"), "FTDI");
        assert_eq!(guess_port_type("ftdi"), "FTDI");
    }

    #[test]
    fn test_guess_port_type_cp210x() {
        assert_eq!(guess_port_type("COM5 (CP2102)"), "CP210x");
        assert_eq!(guess_port_type("cp210"), "CP210x");
    }

    #[test]
    fn test_guess_port_type_unknown() {
        assert_eq!(guess_port_type("COM1"), "Unknown");
        assert_eq!(guess_port_type("COM99"), "Unknown");
    }

    #[test]
    fn test_port_info_from_serial_port_info() {
        let ports = list_ports();
        for port in ports {
            assert!(!port.name.is_empty());
            assert!(!port.port_type.is_empty());
        }
    }
}
