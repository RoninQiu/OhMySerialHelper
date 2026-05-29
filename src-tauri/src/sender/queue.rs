use serde::{Deserialize, Serialize};

/// 发送命令结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendCommand {
    pub id: String,
    pub content: Vec<u8>,
    pub priority: u8,
    pub interval_ms: u64,
}

/// 发送队列
pub struct SendQueue {
    commands: Vec<SendCommand>,
    is_polling: bool,
}

impl SendQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            is_polling: false,
        }
    }

    /// 添加命令（按优先级排序）
    pub fn add(&mut self, cmd: SendCommand) {
        self.commands.push(cmd);
        self.commands.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 移除命令
    pub fn remove(&mut self, id: &str) {
        self.commands.retain(|c| c.id != id);
    }

    /// 开始轮询
    pub fn start_polling(&mut self) {
        self.is_polling = true;
    }

    /// 停止轮询
    pub fn stop_polling(&mut self) {
        self.is_polling = false;
    }

    /// 是否正在轮询
    pub fn is_polling(&self) -> bool {
        self.is_polling
    }

    /// 获取下一个命令
    pub fn next_command(&self) -> Option<&SendCommand> {
        self.commands.first()
    }

    /// 获取所有命令
    pub fn get_commands(&self) -> &[SendCommand] {
        &self.commands
    }

    /// 清空所有命令
    pub fn clear(&mut self) {
        self.commands.clear();
        self.is_polling = false;
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for SendQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_queue_add_and_sort() {
        let mut queue = SendQueue::new();
        queue.add(SendCommand {
            id: "1".to_string(),
            content: vec![0x01],
            priority: 1,
            interval_ms: 100,
        });
        queue.add(SendCommand {
            id: "2".to_string(),
            content: vec![0x02],
            priority: 100,
            interval_ms: 100,
        });

        // 高优先级应该在前面
        let cmds = queue.get_commands();
        assert_eq!(cmds[0].id, "2");
        assert_eq!(cmds[1].id, "1");
    }

    #[test]
    fn test_send_queue_remove() {
        let mut queue = SendQueue::new();
        queue.add(SendCommand {
            id: "1".to_string(),
            content: vec![],
            priority: 50,
            interval_ms: 100,
        });
        queue.add(SendCommand {
            id: "2".to_string(),
            content: vec![],
            priority: 50,
            interval_ms: 100,
        });

        queue.remove("1");
        assert_eq!(queue.get_commands().len(), 1);
        assert_eq!(queue.get_commands()[0].id, "2");
    }

    #[test]
    fn test_send_queue_polling() {
        let mut queue = SendQueue::new();
        assert!(!queue.is_polling());

        queue.start_polling();
        assert!(queue.is_polling());

        queue.stop_polling();
        assert!(!queue.is_polling());
    }
}
