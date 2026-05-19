use std::borrow::Cow;

use anyhow::Result;

use crate::aa;
use crate::coding_agents;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Board {
    Aa,
    AaAgents,
}

const ALL_BOARDS: [Board; 2] = [Board::Aa, Board::AaAgents];

impl Board {
    pub fn all() -> &'static [Board] {
        &ALL_BOARDS
    }

    pub fn label(self) -> Cow<'static, str> {
        match self {
            Board::Aa => Cow::Borrowed("AA"),
            Board::AaAgents => Cow::Borrowed("AA Agents"),
        }
    }

    pub fn shortcut(idx: usize) -> Option<char> {
        match idx {
            0 => Some('1'),
            1 => Some('2'),
            _ => None,
        }
    }

    pub fn from_shortcut(c: char) -> Option<usize> {
        match c {
            '1' => Some(0),
            '2' => Some(1),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    Aa(Vec<aa::Model>),
    AaAgents(Vec<coding_agents::AgentRow>),
}

#[derive(Debug, Clone)]
pub enum Status {
    Loading,
    Loaded(Data),
    Error(String),
}

pub fn fetch(board: Board) -> Result<Data> {
    match board {
        Board::Aa => aa::fetch().map(Data::Aa),
        Board::AaAgents => coding_agents::fetch().map(Data::AaAgents),
    }
}
