use crate::binary::BinaryWriter;

pub struct FileWriter {
    pub data: Vec<u8>,
    current_position: usize,
    section_start: usize,
}

impl Default for FileWriter {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            current_position: 0,
            section_start: 0,
        }
    }
}

impl FileWriter {

    pub fn set_pos(&mut self, pos: usize) {
        self.current_position = pos;
    }

    pub fn mark_section_start(&mut self) {
        self.section_start = self.current_position;
    }

    pub fn current_offset(&self) -> usize {
        self.current_position - self.section_start
    }

}

impl BinaryWriter for FileWriter {
    fn pos(&self) -> usize {
        self.current_position
    }

    fn reserve(&mut self, amount: usize) {
        self.current_position += amount;
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.write_bytes_at(self.current_position, data);
        self.current_position += data.len();
    }

    fn write_bytes_at(&mut self, index: usize, data: &[u8]) {
        if index + data.len() > self.data.len() {
            self.data.resize(index + data.len(), 0);
        }
        self.data[index..index + data.len()].copy_from_slice(data);
    }
}