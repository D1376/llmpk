use anyhow::Result;

use crate::aa;
use crate::arena;
use crate::coding_agents;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Board {
    Aa,
    AaAgents,
    Arena(arena::Slug),
}

impl Board {
    pub fn all() -> Vec<Board> {
        let mut v = vec![Board::Aa, Board::AaAgents];
        for s in arena::Slug::ALL {
            v.push(Board::Arena(s));
        }
        v
    }

    pub fn label(self) -> String {
        match self {
            Board::Aa => "AA".to_string(),
            Board::AaAgents => "AA Agents".to_string(),
            Board::Arena(s) => format!("Arena {}", s.label()),
        }
    }

    pub fn shortcut(self, idx: usize) -> Option<char> {
        match idx {
            0..=8 => char::from_digit((idx as u32) + 1, 10),
            9 => Some('0'),
            10 => Some('-'),
            11 => Some('='),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    Aa(Vec<aa::Model>),
    AaAgents(Vec<coding_agents::AgentRow>),
    Arena(Vec<arena::Entry>),
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
        Board::Arena(s) => arena::fetch(s).map(Data::Arena),
    }
}
