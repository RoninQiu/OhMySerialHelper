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
///
/// 命令列表**始终保持 priority 降序**（`add` 时排序），因此轮转一圈的顺序
/// 就是优先级从高到低。`cursor` 是「下一个要发的下标」，取用时按列表长度回绕。
pub struct SendQueue {
    commands: Vec<SendCommand>,
    is_polling: bool,
    cursor: usize,
}

impl SendQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            is_polling: false,
            cursor: 0,
        }
    }

    /// 添加命令（按优先级排序，并把轮转游标归零）
    pub fn add(&mut self, cmd: SendCommand) {
        self.commands.push(cmd);
        self.commands.sort_by_key(|c| std::cmp::Reverse(c.priority));
        self.cursor = 0;
    }

    /// 移除命令，并把轮转游标归零
    pub fn remove(&mut self, id: &str) {
        self.commands.retain(|c| c.id != id);
        self.cursor = 0;
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

    /// 取下一个要发送的命令，并把游标推进一格。
    ///
    /// 调用方拿到的是**下一个 tick 该发的**那条；是否真的发了出去由调用方
    /// 决定——若写入失败，调用方可以用 `rewind()` 把游标退回，让下一 tick
    /// 重试同一条，而不是静默跳过。
    pub fn next_command(&mut self) -> Option<&SendCommand> {
        if self.commands.is_empty() {
            return None;
        }
        let len = self.commands.len();
        if self.cursor >= len {
            self.cursor = 0;
        }
        let idx = self.cursor;
        self.cursor = (idx + 1) % len;
        self.commands.get(idx)
    }

    /// 把游标退回一格——写入失败时调用，让下一 tick 重试同一条命令
    pub fn rewind(&mut self) {
        if self.commands.is_empty() {
            return;
        }
        let len = self.commands.len();
        self.cursor = if self.cursor == 0 {
            len - 1
        } else {
            self.cursor - 1
        };
    }

    /// 当前游标位置（测试与诊断用）
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// 获取所有命令
    pub fn get_commands(&self) -> &[SendCommand] {
        &self.commands
    }

    /// 清空所有命令，并把游标归零。
    ///
    /// **不影响轮询状态** —— 要停止请用 `stop_polling()`。
    ///
    /// 历史版本这里会顺带把 `is_polling` 置 false，而宿主同步队列的写法正是
    /// `clear()` + 逐条 `add()`，于是每次同步都会把正在跑的 poller 踢出线程。
    pub fn clear(&mut self) {
        self.commands.clear();
        self.cursor = 0;
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

    // ==================== 轮转游标 ====================

    fn cmd(id: &str, priority: u8) -> SendCommand {
        SendCommand {
            id: id.to_string(),
            content: vec![0x01],
            priority,
            interval_ms: 100,
        }
    }

    /// 取三条命令时，`ids()` 按 priority 降序返回它们的 id
    fn ids(queue: &SendQueue) -> Vec<String> {
        queue.get_commands().iter().map(|c| c.id.clone()).collect()
    }

    #[test]
    fn cursor_advances_and_wraps() {
        let mut q = SendQueue::new();
        q.add(cmd("a", 10));
        q.add(cmd("b", 20));
        q.add(cmd("c", 30));

        // priority 降序 → c, b, a
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(q.next_command().unwrap().id.clone());
        }
        assert_eq!(seen, vec!["c", "b", "a", "c"], "取满一轮后应回到队首");
    }

    #[test]
    fn add_resets_cursor_to_zero() {
        let mut q = SendQueue::new();
        q.add(cmd("a", 10));
        q.add(cmd("b", 20));
        assert_eq!(q.next_command().unwrap().id, "b"); // 游标 → 1

        // add 会按 priority 重排，旧游标已指向别的命令 → 归零
        q.add(cmd("c", 99));
        assert_eq!(
            q.next_command().unwrap().id,
            "c",
            "新增后应从 priority 最高那条重新开始"
        );
    }

    #[test]
    fn remove_resets_cursor_and_never_goes_out_of_bounds() {
        let mut q = SendQueue::new();
        q.add(cmd("a", 10));
        q.add(cmd("b", 20));
        q.add(cmd("c", 30));
        assert_eq!(q.next_command().unwrap().id, "c"); // 游标 → 1（"b"）

        // 删掉游标正指着的那条。若不归零，游标 1 会指向 "a"；
        // 归零后应重新从 priority 最高的 "c" 开始。
        q.remove("b");
        assert_eq!(ids(&q), vec!["c", "a"]);
        assert_eq!(q.next_command().unwrap().id, "c", "删除后应重新从队首开始");

        // 连取超过剩余条数也不能越界 / panic
        for _ in 0..5 {
            assert!(q.next_command().is_some());
        }
    }

    #[test]
    fn rewind_retries_the_same_command() {
        let mut q = SendQueue::new();
        q.add(cmd("a", 10));
        q.add(cmd("b", 20));

        assert_eq!(q.next_command().unwrap().id, "b");
        q.rewind(); // 写入失败 → 退回
        assert_eq!(q.next_command().unwrap().id, "b", "rewind 后应重试同一条");
        assert_eq!(q.next_command().unwrap().id, "a");
        q.rewind(); // 从队首退回应绕到队尾
        assert_eq!(q.next_command().unwrap().id, "a");
    }

    #[test]
    fn rewind_on_empty_queue_is_noop() {
        let mut q = SendQueue::new();
        q.rewind();
        assert_eq!(q.cursor(), 0);
        assert!(q.next_command().is_none());
    }

    #[test]
    fn clear_does_not_stop_polling() {
        let mut q = SendQueue::new();
        q.add(cmd("a", 10));
        q.start_polling();

        q.clear();

        // 历史版本这里会把 is_polling 一起置 false。而宿主同步队列的写法正是
        // clear() + 逐条 add()，于是每次同步都把正在跑的 poller 踢出线程。
        assert!(q.is_polling(), "clear() 只该清命令，不该动轮询状态");
        assert!(q.is_empty());
    }

    #[test]
    fn clear_resets_cursor() {
        let mut q = SendQueue::new();
        q.add(cmd("a", 10));
        q.next_command(); // 游标 → 1
        q.clear();
        q.add(cmd("b", 5));
        assert_eq!(q.next_command().unwrap().id, "b", "清空后再加应从队首开始");
    }

    #[test]
    fn empty_queue_next_command_is_none() {
        let mut q = SendQueue::new();
        assert!(q.next_command().is_none());
        // 空队列上连续取也不能 panic
        for _ in 0..3 {
            assert!(q.next_command().is_none());
        }
    }

    #[test]
    fn single_command_repeats_forever() {
        let mut q = SendQueue::new();
        q.add(cmd("only", 1));
        for _ in 0..5 {
            assert_eq!(q.next_command().unwrap().id, "only");
        }
    }

    #[test]
    fn ids_helper_stays_consistent_with_priority_order() {
        let mut q = SendQueue::new();
        q.add(cmd("low", 1));
        q.add(cmd("high", 255));
        q.add(cmd("mid", 128));
        assert_eq!(ids(&q), vec!["high", "mid", "low"]);
    }
}
