use serde::{Deserialize, Serialize};
use serialport::{SerialPortInfo, SerialPortType};

/// 串口信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    /// 串口名称 (如 "COM3")
    pub name: String,
    /// 串口类型（"CH340"/"FTDI"/"CP210x"/"PL2303"/"MCP"/"XR21V"/"TUSB3410"/"PCI"/"Bluetooth"）。
    /// 空串表示无法识别，前端不显示后缀。
    pub port_type: String,
    /// OS 报告的 USB 厂商名（如 "wch.cn"），仅在 USB 端口且驱动注册了 string descriptor 时存在
    pub manufacturer: Option<String>,
    /// OS 报告的 USB 产品名
    pub product: Option<String>,
    /// USB VID（u16，0x0000-0xFFFF）
    pub vid: Option<u16>,
    /// USB PID
    pub pid: Option<u16>,
}

impl From<&SerialPortInfo> for PortInfo {
    fn from(info: &SerialPortInfo) -> Self {
        // 上游字段名 `port_type` 是 SerialPortType 枚举，与本地字段同名，
        // 这里 `info.port_type` 显式指向上游字段。
        let (chip, manufacturer, product, vid, pid) = match &info.port_type {
            SerialPortType::UsbPort(usb) => (
                chip_from_vid_pid(usb.vid, usb.pid)
                    .map(str::to_string)
                    .unwrap_or_default(),
                usb.manufacturer.clone(),
                usb.product.clone(),
                Some(usb.vid),
                Some(usb.pid),
            ),
            SerialPortType::PciPort => ("PCI".to_string(), None, None, None, None),
            SerialPortType::BluetoothPort => {
                ("Bluetooth".to_string(), None, None, None, None)
            }
            SerialPortType::Unknown => (String::new(), None, None, None, None),
        };

        // 兜底：名字里若带可识别关键字（虚拟串口 com0com 之类），仍按名字匹配
        let chip = if chip.is_empty() {
            guess_port_type(&info.port_name)
        } else {
            chip
        };

        Self {
            name: info.port_name.clone(),
            port_type: chip,
            manufacturer,
            product,
            vid,
            pid,
        }
    }
}

/// USB VID/PID → 常见 USB-UART 芯片名
fn chip_from_vid_pid(vid: u16, pid: u16) -> Option<&'static str> {
    match vid {
        0x1A86 => Some("CH340"),  // WCH 全系：CH340/CH341/CH9101F/CH9102/CH9114 等
        0x0403 => Some("FTDI"),   // FTDI 全系
        0x10C4 => Some("CP210x"), // Silicon Labs CP2102/CP2104/CP2105/CP2110
        0x067B => Some("PL2303"), // Prolific
        0x04D8 => Some("MCP"),    // Microchip MCP2200/MCP2221
        0x04E2 => Some("XR21V"),  // Exar XR21V1410
        0x0451 if pid == 0x3410 => Some("TUSB3410"),
        _ => None,
    }
}

/// 根据端口名称猜测串口芯片类型（名字 fallback，识别不到返回空串）
fn guess_port_type(name: &str) -> String {
    let upper = name.to_uppercase();
    if upper.contains("CH340") {
        "CH340".to_string()
    } else if upper.contains("FTDI") || upper.contains("FT232") {
        "FTDI".to_string()
    } else if upper.contains("CP210") || upper.contains("SILICON") {
        "CP210x".to_string()
    } else if upper.contains("PL2303") || upper.contains("PROLIFIC") {
        "PL2303".to_string()
    } else {
        String::new() // 不再返回 "Unknown"
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
    fn test_guess_port_type_empty_when_no_keyword() {
        // v1.0.1 行为变更：识别不到时返回空串，前端据此不显示后缀
        assert_eq!(guess_port_type("COM1"), "");
        assert_eq!(guess_port_type("COM99"), "");
    }

    #[test]
    fn test_chip_from_vid_pid_ch340() {
        assert_eq!(chip_from_vid_pid(0x1A86, 0x7523), Some("CH340"));
        assert_eq!(chip_from_vid_pid(0x1A86, 0x55D4), Some("CH340")); // CH9102
    }

    #[test]
    fn test_chip_from_vid_pid_ftdi() {
        assert_eq!(chip_from_vid_pid(0x0403, 0x6001), Some("FTDI"));
        assert_eq!(chip_from_vid_pid(0x0403, 0x6015), Some("FTDI"));
    }

    #[test]
    fn test_chip_from_vid_pid_cp210x() {
        assert_eq!(chip_from_vid_pid(0x10C4, 0xEA60), Some("CP210x"));
        assert_eq!(chip_from_vid_pid(0x10C4, 0xEA80), Some("CP210x")); // CP2110
    }

    #[test]
    fn test_chip_from_vid_pid_unknown() {
        assert_eq!(chip_from_vid_pid(0x1234, 0x5678), None);
        assert_eq!(chip_from_vid_pid(0x05AC, 0x1234), None); // 苹果等
    }

    #[test]
    fn test_port_info_from_serial_port_info() {
        // v1.0.1：port_type 空串是合法值（无法识别时）；仅校验 name 必填
        let ports = list_ports();
        for port in ports {
            assert!(!port.name.is_empty());
        }
    }
}
