/// 64KB 环形缓冲区
pub struct RingBuffer {
    buf: Vec<u8>,
    capacity: usize,
    write_pos: usize,
    read_pos: usize,
    overflow_count: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            capacity,
            write_pos: 0,
            read_pos: 0,
            overflow_count: 0,
        }
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        let available = self.available_space();
        if data.len() > available {
            self.overflow_count += data.len() - available;
        }

        let to_write = data.len().min(self.capacity);
        for i in 0..to_write {
            self.buf[(self.write_pos + i) % self.capacity] = data[i];
        }
        self.write_pos = (self.write_pos + to_write) % self.capacity;
        to_write
    }

    pub fn read(&mut self, len: usize) -> Vec<u8> {
        let available = self.data_len();
        let to_read = len.min(available);
        let mut result = Vec::with_capacity(to_read);
        for _ in 0..to_read {
            result.push(self.buf[self.read_pos]);
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }
        result
    }

    pub fn data_len(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - self.read_pos + self.write_pos
        }
    }

    pub fn available_space(&self) -> usize {
        self.capacity - self.data_len()
    }

    pub fn water_level(&self) -> f32 {
        self.data_len() as f32 / self.capacity as f32
    }

    pub fn overflow_count(&self) -> usize {
        self.overflow_count
    }

    pub fn reset_overflow(&mut self) {
        self.overflow_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read() {
        let mut buf = RingBuffer::new(64);
        let written = buf.write(b"hello");
        assert_eq!(written, 5);
        assert_eq!(buf.data_len(), 5);
    }

    #[test]
    fn test_wrap_around() {
        let mut buf = RingBuffer::new(10);
        buf.write(b"0123456789");
        assert_eq!(buf.data_len(), 10);
        buf.read(5);
        assert_eq!(buf.data_len(), 5);
        buf.write(b"ABCDEF");
        assert_eq!(buf.data_len(), 11); // overflow 1 byte
        assert_eq!(buf.overflow_count(), 1);
    }

    #[test]
    fn test_water_level() {
        let mut buf = RingBuffer::new(100);
        assert_eq!(buf.water_level(), 0.0);
        buf.write(b"test");
        assert_eq!(buf.water_level(), 0.04);
    }
}
