//! Game logic.

use std::iter;
use std::time::Duration;

use boxy as b;

use rand::seq::SliceRandom;
use rand::Rng;

use crate::term::render::Layer;
use crate::term::texel::Color;
use crate::term::texel::FromChar;
use crate::term::texel::Style;
use crate::term::texel::Texel;
use crate::term::texel::Weight;
use crate::term::tty::Cell;
use crate::term::tty::Event;
use crate::term::tty::Key;

pub struct Options {
  pub board_dims: (u32, u32),
  pub max_card_value: u8,
}

#[derive(Copy, Clone, Debug, Default)]
struct Card {
  /// The value of the card from 0 to 9; zero is a Voltorb.
  value: u8,
  /// Whether this card has been flipped.
  flipped: bool,
  /// By how many texels on each side to compress the card (for flipping).
  compress: u8,
  /// Which memo values have been set by the player for this card.
  memo: u16,
}

pub struct Game {
  options: Options,

  level: u32,
  score: u64,
  round_score: u64,

  cards: Vec<Card>,
  col_hints: Vec<(u32, u32)>,
  row_hints: Vec<(u32, u32)>,

  selected_card: usize,
  state: State,
}

#[derive(Clone, Copy, Debug)]
enum State {
  NewGame(u32),
  Standby,
  Flipping,
  GameOver(u32),
  LevelUp,
}

pub enum Response {
  Wait {
    duration: Duration,
    ignore_inputs: bool,
  },
  WaitForInput,
  Quit,
}

impl Game {
  pub fn new(options: Options) -> Self {
    let (x, y) = options.board_dims;
    Self {
      level: 1,
      score: 0,
      round_score: 0,

      cards: vec![Card::default(); (x as usize) * (y as usize)],
      col_hints: vec![(0, 0); x as usize],
      row_hints: vec![(0, 0); y as usize],

      selected_card: 0,
      state: State::NewGame(1),

      options,
    }
  }

  pub fn generate_board(&mut self) {
    let max_card = self.options.max_card_value as u32;
    let avg_width = (self.options.board_dims.0 + self.options.board_dims.1) / 2;

    let mut rng = rand::thread_rng();
    let voltorbs = (self.cards.len() / 5 * 2)
      .min((self.level * (avg_width - 3)) as usize + self.cards.len() / 5);

    let mut sum = self.level * (max_card - 1) + 3 * max_card;
    if rng.gen::<bool>() {
      sum -= (max_card - 1) / 2;
    }
    let maxes = [1u64, 2, 4, 8, 12, 20, 40, 80];
    let max = maxes[self.level as usize - 1]
      * (1 << (max_card - 1))
      * (self.cards.len() as u64).pow(max_card / 2);

    let mut cards = Vec::new();
    let mut coins = 1;
    while sum > 1 && cards.len() < self.cards.len() - voltorbs {
      let max_candidate =
        (2..=max_card).filter(|x| (*x as u64) * coins <= max).max();
      if max_candidate.is_none() {
        break;
      }

      let value = rng.gen_range(2..=max_candidate.unwrap());
      coins *= value as u64;
      cards.push(value);
    }

    self.cards.fill(Card {
      value: 1,
      ..Card::default()
    });
    let mut indices = (0usize..self.cards.len()).collect::<Vec<_>>();
    indices.shuffle(&mut rng);
    for index in &indices[0..voltorbs] {
      self.cards[*index].value = 0;
    }
    for (index, card) in Iterator::zip(indices[voltorbs..].iter(), cards.iter())
    {
      self.cards[*index].value = *card as u8;
    }

    self.round_score = 0;
    self.row_hints.fill((0, 0));
    self.col_hints.fill((0, 0));

    let stride = self.options.board_dims.0 as usize;
    for (i, card) in self.cards.iter().enumerate() {
      self.row_hints[i / stride].0 += card.value as u32;
      self.col_hints[i % stride].0 += card.value as u32;
      if card.value == 0 {
        self.row_hints[i / stride].1 += 1;
        self.col_hints[i % stride].1 += 1;
      }
    }
  }

  /// Presents a player interaction to the game, indicating to the main loop
  /// how to respond in kind.
  pub fn interact(&mut self, event: Option<Event>) -> Response {
    let stride = self.options.board_dims.0 as usize;
    match (self.state, event) {
      (
        _,
        Some(Event::Key {
          key: Key::Glyph('q'),
          ..
        }),
      ) => return Response::Quit,
      (State::NewGame(level), _) => {
        let mut done = true;
        for card in &mut self.cards {
          if !card.flipped && card.compress > 0 {
            card.compress -= 1;
            if card.compress != 0 {
              done = false;
            }
          } else if card.flipped {
            card.compress += 1;
            if card.compress == 4 {
              card.flipped = false;
            }
            done = false;
          }
        }
        if !done {
          return Response::Wait {
            duration: Duration::from_millis(10),
            ignore_inputs: true,
          };
        }

        self.level = level.max(1);
        self.generate_board();
        self.state = State::Standby;
      }
      (State::Standby, Some(Event::Key { key, .. })) => match key {
        Key::Left => {
          if self.selected_card % stride == 0 {
            self.selected_card += stride - 1;
          } else {
            self.selected_card -= 1;
          }
        }
        Key::Right => {
          self.selected_card += 1;
          if self.selected_card % stride == 0 {
            self.selected_card -= stride;
          }
        }
        Key::Up => {
          if let Some(select) = self.selected_card.checked_sub(stride) {
            self.selected_card = select;
          } else {
            self.selected_card =
              self.cards.len() - (stride - self.selected_card % stride);
          }
        }
        Key::Down => {
          self.selected_card += stride;
          if self.selected_card > self.cards.len() {
            self.selected_card %= stride;
          }
        }
        Key::Enter => {
          if !self.cards[self.selected_card].flipped {
            self.state = State::Flipping;
            self.cards[self.selected_card].compress += 1;
            return Response::Wait {
              duration: Duration::from_millis(10),
              ignore_inputs: true,
            };
          }
        }
        Key::Glyph(k @ '0'..='9') => {
          let index = k as u8 - b'0';
          self.cards[self.selected_card].memo ^= 1 << index;
        }
        _ => {}
      },
      (State::Flipping, _) => {
        let card = &mut self.cards[self.selected_card];
        if card.flipped {
          card.compress -= 1;
          if card.compress == 0 {
            // Resolve actual game logic now that the flip has happened.
            if card.value == 0 {
              let flipped = self.cards.iter().filter(|x| x.flipped).count();
              self.state =
                State::GameOver((flipped as u32 - 1).min(self.level));
              return Response::Wait {
                duration: Duration::from_millis(500),
                ignore_inputs: false,
              };
            }

            if self.round_score == 0 {
              self.round_score = 1;
            }
            self.round_score *= card.value as u64;

            // Check if we've won.
            if !self
              .cards
              .iter()
              .any(|card| card.value > 1 && !card.flipped)
            {
              self.state = State::LevelUp;
              return Response::Wait {
                duration: Duration::from_millis(500),
                ignore_inputs: false,
              };
            }

            self.state = State::Standby;
            return Response::WaitForInput;
          }
        } else {
          card.compress += 1;
          if card.compress == 4 {
            card.flipped = true;
          }
        }
        return Response::Wait {
          duration: Duration::from_millis(10),
          ignore_inputs: true,
        };
      }
      (State::GameOver(level), _) => {
        let mut done = true;
        for card in &mut self.cards {
          if card.flipped && card.compress > 0 {
            card.compress -= 1;
            if card.compress != 0 {
              done = false;
            }
          } else if !card.flipped {
            card.compress += 1;
            if card.compress == 4 {
              card.flipped = true;
            }
            done = false;
          }
        }
        if !done {
          return Response::Wait {
            duration: Duration::from_millis(10),
            ignore_inputs: true,
          };
        }

        self.state = State::NewGame(level);
        return Response::Wait {
          duration: Duration::from_secs(5),
          ignore_inputs: false,
        };
      }
      (State::LevelUp, _) => {
        self.score = self.round_score;
        self.state = State::NewGame((self.level + 1).min(8));
        return Response::Wait {
          duration: Duration::from_secs(5),
          ignore_inputs: false,
        };
      }
      _ => {}
    }

    Response::WaitForInput
  }

  pub fn render(&self, viewport: Cell) -> Vec<Layer<'static>> {
    let mut layers = Vec::new();

    let green = Style::new().with_fg(Color::LtGreen);
    let gold = Style::new().with_fg(Color::DkYellow);

    let (width, height) = self.options.board_dims;
    let width = width as usize;
    let height = height as usize;

    // First, draw the cards.
    for (i, card) in self.cards.iter().enumerate() {
      // Cards are 9x5.
      // ╭───────╮ ╭───────╮ ╭───────╮ ╭───────╮ ╭───────╮
      // │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │
      // │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │
      // │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │
      // ╰───────╯ ╰───────╯ ╰───────╯ ╰───────╯ ╰───────╯
      let weight = if i == self.selected_card {
        b::Weight::Doubled
      } else {
        b::Weight::Normal
      };
      let mut card_art = vec![
        b::Char::upper_left(weight)
          .style(b::Style::Curved)
          .with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::upper_right(weight)
          .style(b::Style::Curved)
          .with_style(green),
        //
        b::Char::vertical(weight).with_style(green),
        Texel::empty(),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        Texel::empty(),
        b::Char::vertical(weight).with_style(green),
        //
        b::Char::vertical(weight).with_style(green),
        Texel::empty(),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        Texel::empty(),
        b::Char::vertical(weight).with_style(green),
        //
        b::Char::vertical(weight).with_style(green),
        Texel::empty(),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        '╱'.with_style(green),
        Texel::empty(),
        b::Char::vertical(weight).with_style(green),
        //
        b::Char::lower_left(weight)
          .style(b::Style::Curved)
          .with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::lower_right(weight)
          .style(b::Style::Curved)
          .with_style(green),
      ];

      // For each card, if it's been flipped, we draw the contents in the
      // inner 5x3 box; this is either a number or a Voltorb; otherwise, we
      // draw the memos.
      if card.flipped {
        draw_card(&mut card_art, card.value);
      } else {
        for i in 0..=9 {
          let row = i / 3 + 1;
          let col = i % 3 * 2 + 2;
          let idx = col + row * 9;
          let char = match i {
            0 => 'o',
            i => (b'0' + (i as u8)) as char,
          };

          if card.memo & (1 << i) != 0 {
            card_art[idx] = Texel::new(char).with_style(gold);
          }
        }
      }

      if card.compress > 0 {
        let compress = card.compress as usize;
        for row in card_art.chunks_mut(9) {
          row[compress] = row[0];
          row[8 - compress] = row[8];
          for i in 0..compress {
            row[i] = Texel::empty();
            row[8 - i] = Texel::empty();
          }
        }
      }

      layers.push(Layer {
        origin: Cell::from_xy(10 * (i % width), 5 * (i / width)),
        stride: 9,
        data: card_art.into(),
      })
    }

    fn make_hint(sum: u32, voltorbs: u32) -> Vec<Texel> {
      let green = Style::new().with_fg(Color::LtGreen);
      let red = Style::new().with_fg(Color::LtRed);
      let wht = Style::new().with_fg(Color::LtWhite);
      let blu = Style::new().with_fg(Color::LtBlue);
      let weight = b::Weight::Normal;
      let mut hint = vec![
        b::Char::upper_left(weight)
          .style(b::Style::Curved)
          .with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::upper_right(weight)
          .style(b::Style::Curved)
          .with_style(green),
        //
        b::Char::vertical(weight).with_style(green),
        Texel::empty(),
        'x'.with_style(blu),
        'x'.with_style(blu),
        'x'.with_style(blu),
        'x'.with_style(blu),
        'x'.with_style(blu),
        Texel::empty(),
        b::Char::vertical(weight).with_style(green),
        //
        b::Char::vertical(weight).with_style(green),
        Texel::empty(),
        '▄'.with_style(red),
        '▄'.with_style(red),
        Texel::empty(),
        b::Char::horizontal(b::Weight::Doubled).with_style(green),
        b::Char::horizontal(b::Weight::Doubled).with_style(green),
        Texel::empty(),
        b::Char::vertical(weight).with_style(green),
        //
        b::Char::vertical(weight).with_style(green),
        Texel::empty(),
        '▀'.with_style(wht),
        '▀'.with_style(wht),
        Texel::empty(),
        'x'.with_style(red),
        'x'.with_style(red),
        Texel::empty(),
        b::Char::vertical(weight).with_style(green),
        //
        b::Char::lower_left(weight)
          .style(b::Style::Curved)
          .with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::horizontal(weight).with_style(green),
        b::Char::lower_right(weight)
          .style(b::Style::Curved)
          .with_style(green),
      ];

      for (i, tx) in blu
        .texels_from_str(&format!("{sum:>5}"))
        .into_iter()
        .enumerate()
      {
        hint[9 + 2 + i] = tx;
      }

      for (i, tx) in red
        .texels_from_str(&format!("{voltorbs:>3}"))
        .into_iter()
        .enumerate()
      {
        hint[3 * 9 + 4 + i] = tx;
      }

      hint
    }

    // Next, draw the hints along each side.
    for (i, &(sum, voltorbs)) in self.col_hints.iter().enumerate() {
      layers.push(Layer {
        origin: Cell::from_xy(10 * (i % width), 5 * height + 1),
        stride: 9,
        data: make_hint(sum, voltorbs).into(),
      })
    }

    for (i, &(sum, voltorbs)) in self.row_hints.iter().enumerate() {
      layers.push(Layer {
        origin: Cell::from_xy(10 * width + 1, 5 * i),
        stride: 9,
        data: make_hint(sum, voltorbs).into(),
      })
    }

    // Draw the level number.
    let mut level = vec![Texel::empty(); 9 * 5];
    draw_card(&mut level, self.level as u8);
    layers.push(Layer {
      origin: Cell::from_xy(10 * width + 1, 5 * height + 1),
      stride: 9,
      data: level.into(),
    });

    let width_cards_tx = 10 * (width + 1);

    // Draw the controls/scoreboard.
    let mut controls = Vec::new();
    let bar = iter::repeat(b::Char::horizontal(b::Weight::Doubled).into_char())
      .take(32)
      .collect::<String>();
    controls.extend(gold.with_weight(Weight::Bold).texels_from_str(&bar));
    controls.extend(gold.texels_from_str(" Arrows: Move  ╱╱   0-9: Memo   "));
    controls.extend(gold.texels_from_str(" Enter:  Flip  ╱╱   `q`: Quit   "));
    controls.extend(gold.with_weight(Weight::Bold).texels_from_str(&bar));
    controls.extend(
      gold
        .with_weight(Weight::Bold)
        .texels_from_str("     Coins     ╱╱     Total     "),
    );
    controls.extend(gold.texels_from_str(&format!(
      " {:013} ╱╱ {:013} ",
      self.round_score, self.score
    )));
    controls.extend(gold.with_weight(Weight::Bold).texels_from_str(&bar));
    layers.push(Layer {
      origin: Cell::from_xy(width_cards_tx + 2, 0),
      stride: 32,
      data: controls.into(),
    });

    // Center everything.
    let (_, lower) = Layer::bounding_box(&layers);
    let offset_x = viewport.col().saturating_sub(lower.col()) / 2;
    let offset_y = viewport.row().saturating_sub(lower.row()) / 2;

    for layer in &mut layers {
      layer.origin = Cell::from_xy(
        layer.origin.col() + offset_x,
        layer.origin.row() + offset_y,
      );
    }

    layers
  }
}

fn draw_card(card: &mut Vec<Texel>, n: u8) {
  assert!(n < 10);
  for (i, line) in card.chunks_mut(9).skip(1).take(3).enumerate() {
    let line = &mut line[2..7];
    let red = Style::new().with_fg(Color::LtRed);
    let wht = Style::new().with_fg(Color::LtWhite);
    let blu = Style::new().with_fg(Color::LtBlue);

    let art = match (n, i) {
      (0, 0) => [
        Texel::empty(),
        '▁'.with_style(red),
        '▁'.with_style(red),
        '▁'.with_style(red),
        Texel::empty(),
      ],
      (0, 1) => [
        Texel::empty(),
        '▛'.with_style(red),
        '█'.with_style(red),
        '▜'.with_style(red),
        Texel::empty(),
      ],
      (0, 2) => [
        Texel::empty(),
        '▀'.with_style(wht),
        '▀'.with_style(wht),
        '▀'.with_style(wht),
        Texel::empty(),
      ],
      (1, 0) => [
        Texel::empty(),
        b::Char::right_half(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
        Texel::empty(),
      ],
      (1, 1) => [
        Texel::empty(),
        Texel::empty(),
        b::Char::vertical(b::Weight::Thick).with_style(blu),
        Texel::empty(),
        Texel::empty(),
      ],
      (1, 2) => [
        Texel::empty(),
        b::Char::right_half(b::Weight::Thick).with_style(blu),
        b::Char::up_tee(b::Weight::Thick).with_style(blu),
        b::Char::left_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (2, 0) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (2, 1) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::lower_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (2, 2) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::left_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (3, 0) => [
        Texel::empty(),
        b::Char::right_half(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (3, 1) => [
        Texel::empty(),
        Texel::empty(),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::left_tee(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (3, 2) => [
        Texel::empty(),
        b::Char::right_half(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::lower_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (4, 0) => [
        Texel::empty(),
        b::Char::down_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
        b::Char::down_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (4, 1) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::cross(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (4, 2) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::up_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (5, 0) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::left_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (5, 1) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (5, 2) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::lower_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (6, 0) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (6, 1) => [
        Texel::empty(),
        b::Char::right_tee(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (6, 2) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::lower_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (7, 0) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (7, 1) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::vertical(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (7, 2) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::up_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (8, 0) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (8, 1) => [
        Texel::empty(),
        b::Char::right_tee(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::left_tee(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (8, 2) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::lower_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (9, 0) => [
        Texel::empty(),
        b::Char::upper_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::upper_right(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (9, 1) => [
        Texel::empty(),
        b::Char::lower_left(b::Weight::Thick).with_style(blu),
        b::Char::horizontal(b::Weight::Thick).with_style(blu),
        b::Char::left_tee(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      (9, 2) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::up_half(b::Weight::Thick).with_style(blu),
        Texel::empty(),
      ],
      x => panic!("bad card data: {x:?}"),
    };
    line.copy_from_slice(&art);
  }
}
