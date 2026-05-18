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

const ALL_BOARDS: [Board; 12] = [
    Board::Aa,
    Board::AaAgents,
    Board::Arena(arena::Slug::Text),
    Board::Arena(arena::Slug::Search),
    Board::Arena(arena::Slug::Vision),
    Board::Arena(arena::Slug::Document),
    Board::Arena(arena::Slug::Code),
    Board::Arena(arena::Slug::TextToImage),
    Board::Arena(arena::Slug::ImageEdit),
    Board::Arena(arena::Slug::TextToVideo),
    Board::Arena(arena::Slug::ImageToVideo),
    Board::Arena(arena::Slug::VideoEdit),
];

impl Board {
    pub fn all() -> &'static [Board] {
        &ALL_BOARDS
    }

    pub fn label(self) -> String {
        match self {
            Board::Aa => "AA".to_string(),
            Board::AaAgents => "AA Agents".to_string(),
            Board::Arena(s) => format!("Arena {}", s.label()),
        }
    }

    pub fn shortcut(idx: usize) -> Option<char> {
        match idx {
            0..=8 => char::from_digit((idx as u32) + 1, 10),
            9 => Some('0'),
            10 => Some('-'),
            11 => Some('='),
            _ => None,
        }
    }

    pub fn from_shortcut(c: char) -> Option<usize> {
        let idx = match c {
            '1'..='9' => (c as u8 - b'1') as usize,
            '0' => 9,
            '-' => 10,
            '=' => 11,
            _ => return None,
        };
        if idx < ALL_BOARDS.len() {
            Some(idx)
        } else {
            None
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
