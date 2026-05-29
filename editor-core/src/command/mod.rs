pub mod edit;
pub mod mode;
pub mod move_command;
pub mod system;

pub use edit::Edit;
pub use move_command::Move;
pub use system::System;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Command {
    Move(Move),
    Edit(Edit),
    System(System),
}
