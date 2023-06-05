use enum_repr::EnumRepr;

// Msg Reply IDs
#[EnumRepr(type = "u64")]
pub enum MsgReplyID {
    TransferIbc = 1,
    SendAddr = 2,
}