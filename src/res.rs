use std::iter::repeat_n;
use crate::{ResourceFile, ResourceType};
use crate::binary::{BinaryWritable, BinaryWriter};


impl ResourceFile {

    fn realign(&mut self) {
        self.align_to(4)
    }

    fn reserve_u32(&mut self) -> usize {
        let pos = self.pos();
        self.write_u32(0);
        pos
    }

    fn update_u32(&mut self, location: usize, v: u32) {
        self.write_bytes_at(location, &v.to_le_bytes())
    }

    fn write_ident(&mut self, id: u16) {
        self.write_u16(0xffff);
        self.write_u16(id);
    }

    pub(crate) fn write_resource<B: BinaryWritable +?Sized>(&mut self, ty: ResourceType, name: u16, data: &B) {
        let header_start = self.pos();
        let data_size_loc = self.reserve_u32();
        let header_size_loc = self.reserve_u32();
        self.write_ident(ty as u16);
        self.write_ident(name);
        self.realign();
        self.write_u32(0); // format version
        self.write_u16(ty.flags());
        self.write_u16(match ty {
            ResourceType::None => 0x0,
            _ => 0x0409 // en-US
        });
        self.write_u32(0); // data version
        self.write_u32(0); // characteristics

        let header_len = self.pos() - header_start;
        self.update_u32(header_size_loc, header_len as u32);
        let data_start = self.pos();
        data.write_to(self);
        let data_len = self.pos() - data_start;
        self.update_u32(data_size_loc, data_len as u32);
        self.realign();
    }

}

impl BinaryWriter for ResourceFile {
    fn pos(&self) -> usize {
        self.0.len()
    }

    fn reserve(&mut self, amount: usize) {
        self.0.extend(repeat_n(0, amount))
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.0.extend_from_slice(data)
    }

    fn write_bytes_at(&mut self, index: usize, data: &[u8]) {
        self.0[index..(index + data.len())].copy_from_slice(data)
    }
}