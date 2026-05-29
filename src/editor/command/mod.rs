use edit::Edit;
use move_command::Move;
use system::System;

pub mod move_command;
pub mod edit;
pub mod system;
pub mod mode;

#[derive(Copy, Clone)]
pub enum Command {
    Move(Move), Edit(Edit), System(System)
}