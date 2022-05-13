// Graphics functions for `Game`.

use std::iter;

use boxy as b;

use crate::game::Game;
use crate::game::Hint;
use crate::term::render::Layer;
use crate::term::texel::Color;
use crate::term::texel::FromChar;
use crate::term::texel::Style;
use crate::term::texel::Texel;
use crate::term::texel::Weight;
use crate::term::tty::Cell;

// Cards are 9x5; the area that can be drawn on is 5x3, and starts
// at the coordinate (2, 1).
// ╭───────╮ ╭───────╮ ╭───────╮ ╭───────╮ ╭───────╮
// │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │
// │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │
// │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │ │ ╱╱╱╱╱ │
// ╰───────╯ ╰───────╯ ╰───────╯ ╰───────╯ ╰───────╯
const CARD_WIDTH: usize = 9;
const CARD_HEIGHT: usize = 5;
const ART_ORIGIN_WIDTH: usize = 2;
const ART_ORIGIN_HEIGHT: usize = 1;
const ART_WIDTH: usize = CARD_WIDTH - ART_ORIGIN_WIDTH * 2;
const ART_HEIGHT: usize = CARD_HEIGHT - ART_ORIGIN_HEIGHT * 2;

/// Returns an index into a card returned by `new_card` for a coordinate
/// within the art area.
fn art_index(x: usize, y: usize) -> usize {
  (x + ART_ORIGIN_WIDTH) + CARD_WIDTH * (y + ART_ORIGIN_HEIGHT)
}

/// Creates a new blank card.
fn new_card(b_weight: b::Weight, tx_style: Style) -> Vec<Texel> {
  vec![
    b::Char::upper_left(b_weight)
      .style(b::Style::Curved)
      .with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::upper_right(b_weight)
      .style(b::Style::Curved)
      .with_style(tx_style),
    //
    b::Char::vertical(b_weight).with_style(tx_style),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    b::Char::vertical(b_weight).with_style(tx_style),
    //
    b::Char::vertical(b_weight).with_style(tx_style),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    b::Char::vertical(b_weight).with_style(tx_style),
    //
    b::Char::vertical(b_weight).with_style(tx_style),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    Texel::empty(),
    b::Char::vertical(b_weight).with_style(tx_style),
    //
    b::Char::lower_left(b_weight)
      .style(b::Style::Curved)
      .with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::horizontal(b_weight).with_style(tx_style),
    b::Char::lower_right(b_weight)
      .style(b::Style::Curved)
      .with_style(tx_style),
  ]
}

pub fn render(game: &Game, viewport: Cell) -> Vec<Layer<'static>> {
  let mut layers = Vec::new();

  let green = Style::new().with_fg(Color::LtGreen);
  let gold = Style::new().with_fg(Color::DkYellow);

  let (width, height) = game.options.board_dims;
  let width = width as usize;
  let height = height as usize;

  // First, draw the cards.
  for (i, card) in game.cards.iter().enumerate() {
    let weight = if i == game.selected_card {
      b::Weight::Doubled
    } else {
      b::Weight::Normal
    };

    let mut card_art = new_card(weight, green);

    // For each card, if it's been flipped, we draw the contents in the
    // inner 5x3 box; this is either a number or a Voltorb; otherwise, we
    // draw the memos.
    draw_card(&mut card_art, card.flipped.then(|| card.value));
    if card.flipped {
      for i in 0..=9 {
        if card.memo & (1 << i) != 0 {
          let c = match i {
            0 => 'o',
            i => (b'0' + (i as u8)) as char,
          };
          card_art[art_index(i % 3 * 2, i / 3)] =
            Texel::new(c).with_style(gold);
        }
      }
    }

    if card.compress > 0 {
      let compress = card.compress as usize;
      for row in card_art.chunks_mut(CARD_WIDTH) {
        row[compress] = row[0];
        row[CARD_WIDTH - compress - 1] = row[CARD_WIDTH - 1];
        for i in 0..compress {
          row[i] = Texel::empty();
          row[CARD_WIDTH - i - 1] = Texel::empty();
        }
      }
    }

    layers.push(Layer {
      origin: Cell::from_xy(
        (CARD_WIDTH + 1) * (i % width),
        CARD_HEIGHT * (i / width),
      ),
      stride: CARD_WIDTH,
      data: card_art.into(),
    })
  }

  fn make_hint(hint: Hint) -> Vec<Texel> {
    let green = Style::new().with_fg(Color::LtGreen);
    let red = Style::new().with_fg(Color::LtRed);
    let wht = Style::new().with_fg(Color::LtWhite);
    let blu = Style::new().with_fg(Color::LtBlue);
    let mut hint_art = new_card(b::Weight::Normal, green);

    // Draw the small Voltorb.
    hint_art[art_index(0, 1)] = '▄'.with_style(red);
    hint_art[art_index(1, 1)] = '▄'.with_style(red);
    hint_art[art_index(0, 2)] = '▀'.with_style(wht);
    hint_art[art_index(1, 2)] = '▀'.with_style(wht);

    // Draw the bar separating the two numbers.

    hint_art[art_index(3, 1)] =
      b::Char::horizontal(b::Weight::Doubled).with_style(green);
    hint_art[art_index(4, 1)] =
      b::Char::horizontal(b::Weight::Doubled).with_style(green);

    for (i, tx) in blu
      .texels_from_str(&format!("{:>5}", hint.sum))
      .into_iter()
      .enumerate()
    {
      hint_art[art_index(i, 0)] = tx;
    }

    for (i, tx) in red
      .texels_from_str(&format!("{:>3}", hint.voltorbs))
      .into_iter()
      .enumerate()
    {
      hint_art[art_index(i + 2, 2)] = tx;
    }

    hint_art
  }

  // Next, draw the hints along each side.
  for (i, &h) in game.col_hints.iter().enumerate() {
    layers.push(Layer {
      origin: Cell::from_xy(
        (CARD_WIDTH + 1) * (i % width),
        CARD_HEIGHT * height + 1,
      ),
      stride: 9,
      data: make_hint(h).into(),
    })
  }

  for (i, &h) in game.row_hints.iter().enumerate() {
    layers.push(Layer {
      origin: Cell::from_xy((CARD_WIDTH + 1) * width + 1, CARD_HEIGHT * i),
      stride: 9,
      data: make_hint(h).into(),
    })
  }

  // Draw the level number.
  let mut level = vec![Texel::empty(); CARD_WIDTH * CARD_HEIGHT];
  draw_card(&mut level, Some(game.level as u8));
  layers.push(Layer {
    origin: Cell::from_xy(
      (CARD_WIDTH + 1) * width + 1,
      CARD_HEIGHT * height + 1,
    ),
    stride: 9,
    data: level.into(),
  });

  let width_cards_tx = (CARD_WIDTH + 1) * (width + 1);

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
    game.round_score, game.score
  )));
  controls.extend(gold.with_weight(Weight::Bold).texels_from_str(&bar));
  layers.push(Layer {
    origin: Cell::from_xy(width_cards_tx + 2, 0),
    stride: bar.chars().count(),
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

fn draw_card(card: &mut Vec<Texel>, n: Option<u8>) {
  for (i, line) in card
    .chunks_mut(CARD_WIDTH)
    .skip(ART_ORIGIN_HEIGHT)
    .take(ART_HEIGHT)
    .enumerate()
  {
    let line = &mut line[ART_ORIGIN_WIDTH..][..ART_WIDTH];
    let red = Style::new().with_fg(Color::LtRed);
    let wht = Style::new().with_fg(Color::LtWhite);
    let blu = Style::new().with_fg(Color::LtBlue);

    if n.is_none() {
      line.fill('╱'.with_style(Style::new().with_fg(Color::LtGreen)));
      continue;
    }

    let art = match (n.unwrap(), i) {
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
