
#[derive(Debug, Default, Clone, Copy)]
pub struct BufferDataPos {
    pub buf_idx: usize,
    pub offset: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BufferStatus {
    #[default]
    Ready4Submit,
    Ready4Process, // read from it. or write to it
    Invalid,
}