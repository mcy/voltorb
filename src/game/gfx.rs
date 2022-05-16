// Graphics functions for `Game`.

use std::iter;

use boxy as b;

use crate::game::Game;
use crate::game::Hint;
use crate::term::texel::Color;
use crate::term::texel::FromChar;
use crate::term::texel::Style;
use crate::term::texel::Texel;
use crate::term::Cell;
use crate::term::Layer;

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

#[derive(Copy, Clone)]
pub struct Stylesheet {
  card_style: Style,
  card_weight: b::Weight,

  selected_style: Style,
  selected_weight: b::Weight,

  number_style: Style,
  number_weight: b::Weight,

  voltorb_red: Style,
  voltorb_wht: Style,

  coin_style: Style,
  memo_style: Style,

  hint_colors: [Style; 5],
}

impl Default for Stylesheet {
  fn default() -> Stylesheet {
    Self {
      card_style: Color::LtGreen.fg(),
      card_weight: b::Weight::Normal,
      selected_style: Color::LtCyan.fg(),
      selected_weight: b::Weight::Doubled,
      number_style: Color::LtBlue.fg(),
      number_weight: b::Weight::Thick,
      voltorb_red: Color::LtRed.fg(),
      voltorb_wht: Color::LtWhite.fg(),
      coin_style: Color::DkYellow.fg(),
      memo_style: Color::DkYellow.fg(),
      hint_colors: [
        Color::DkRed.fg(),
        Color::DkGreen.fg(),
        Color::DkYellow.fg(),
        Color::DkBlue.fg(),
        Color::DkMagenta.fg(),
      ],
    }
  }
}

pub fn render(
  game: &Game,
  viewport: Cell,
  sheet: &Stylesheet,
) -> Vec<Layer<'static>> {
  let mut layers = Vec::new();

  let (width, height) = game.options.board_dims;
  let width = width as usize;
  let height = height as usize;

  // First, draw the cards.
  for (i, card) in game.cards.iter().enumerate() {
    let mut card_art = new_card(sheet, i == game.selected_card);

    // For each card, if it's been flipped, we draw the contents in the
    // inner 5x3 box; this is either a number or a Voltorb; otherwise, we
    // draw the memos.
    draw_card(
      &mut card_art,
      card.flipped.then(|| card.value),
      i == game.selected_card,
      sheet,
    );

    if card.flipped {
      for i in 0..=9 {
        if card.memo & (1 << i) != 0 {
          let c = match i {
            0 => 'o',
            i => (b'0' + (i as u8)) as char,
          };
          card_art[art_index(i % 3 * 2, i / 3)] =
            Texel::new(c).with_style(sheet.memo_style);
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
    });

    // In debug mode, draw the zero-index of the card in the corner.
    if game.options.enable_debugging {
      layers.push(Layer {
        origin: Cell::from_xy(
          (CARD_WIDTH + 1) * (i % width) + CARD_WIDTH - 2,
          CARD_HEIGHT * (i / width) + CARD_HEIGHT - 1,
        ),
        stride: 2,
        data: sheet
          .memo_style
          .texels_from_str(&format!("{:02}", i))
          .collect::<Vec<_>>()
          .into(),
      });
    }
  }

  fn make_hint(hint: Hint, idx: usize, sheet: &Stylesheet) -> Vec<Texel> {
    let mut sheet = *sheet;
    sheet.card_style = sheet.hint_colors[idx % sheet.hint_colors.len()];
    let mut hint_art = new_card(&sheet, false);

    // Draw the small Voltorb.
    hint_art[art_index(0, 1)] = '▄'.with_style(sheet.voltorb_red);
    hint_art[art_index(1, 1)] = '▄'.with_style(sheet.voltorb_red);
    hint_art[art_index(0, 2)] = '▀'.with_style(sheet.voltorb_wht);
    hint_art[art_index(1, 2)] = '▀'.with_style(sheet.voltorb_wht);

    // Draw the bar separating the two numbers.

    hint_art[art_index(3, 1)] =
      b::Char::horizontal(b::Weight::Doubled).with_style(sheet.voltorb_wht);
    hint_art[art_index(4, 1)] =
      b::Char::horizontal(b::Weight::Doubled).with_style(sheet.voltorb_wht);

    for (i, tx) in sheet
      .voltorb_wht
      .texels_from_str(&format!("{:>5}", hint.sum))
      .into_iter()
      .enumerate()
    {
      hint_art[art_index(i, 0)] = tx;
    }

    for (i, tx) in sheet
      .voltorb_red
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
      data: make_hint(h, i, sheet).into(),
    })
  }

  for (i, &h) in game.row_hints.iter().enumerate() {
    layers.push(Layer {
      origin: Cell::from_xy((CARD_WIDTH + 1) * width + 1, CARD_HEIGHT * i),
      stride: 9,
      data: make_hint(h, i, sheet).into(),
    })
  }

  // Draw the level number.
  let mut level = vec![Texel::empty(); CARD_WIDTH * CARD_HEIGHT];
  draw_card(&mut level, Some(game.level as u8), false, sheet);
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
  controls.extend(sheet.coin_style.texels_from_str(&bar));
  controls.extend(
    sheet
      .coin_style
      .texels_from_str(" [Arrow] Move  ╱╱  [0-9] Memo   "),
  );
  controls.extend(
    sheet
      .coin_style
      .texels_from_str(" [Enter] Flip  ╱╱  [Q]   Quit   "),
  );
  controls.extend(sheet.coin_style.texels_from_str(&bar));
  controls.extend(
    sheet
      .coin_style
      .texels_from_str("     Coins     ╱╱     Total     "),
  );
  controls.extend(sheet.coin_style.texels_from_str(&format!(
    " {:.>13} ╱╱ {:.>13} ",
    game.round_score, game.score
  )));
  controls.extend(sheet.coin_style.texels_from_str(&bar));
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

  for (i, d) in game.debug.iter().enumerate() {
    layers.push(Layer {
      origin: Cell::from_xy(0, i),
      stride: viewport.col(),
      data: sheet
        .voltorb_red
        .texels_from_str(d)
        .collect::<Vec<_>>()
        .into(),
    });
  }

  layers
}

/// Creates a new blank card.
fn new_card(sheet: &Stylesheet, selected: bool) -> Vec<Texel> {
  let (b_weight, tx_style) = if selected {
    (sheet.selected_weight, sheet.selected_style)
  } else {
    (sheet.card_weight, sheet.card_style)
  };

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

/// Draws art on a card: a backside, a Voltorb, or a number.
fn draw_card(
  card: &mut Vec<Texel>,
  n: Option<u8>,
  selected: bool,
  sheet: &Stylesheet,
) {
  for (i, line) in card
    .chunks_mut(CARD_WIDTH)
    .skip(ART_ORIGIN_HEIGHT)
    .take(ART_HEIGHT)
    .enumerate()
  {
    let line = &mut line[ART_ORIGIN_WIDTH..][..ART_WIDTH];

    if n.is_none() {
      line.fill('╱'.with_style(if selected {
        sheet.selected_style
      } else {
        sheet.card_style
      }));
      continue;
    }

    let art = match (n.unwrap(), i) {
      (0, 0) => [
        Texel::empty(),
        '▁'.with_style(sheet.voltorb_red),
        '▁'.with_style(sheet.voltorb_red),
        '▁'.with_style(sheet.voltorb_red),
        Texel::empty(),
      ],
      (0, 1) => [
        Texel::empty(),
        '▛'.with_style(sheet.voltorb_red),
        '█'.with_style(sheet.voltorb_red),
        '▜'.with_style(sheet.voltorb_red),
        Texel::empty(),
      ],
      (0, 2) => [
        Texel::empty(),
        '▀'.with_style(sheet.voltorb_wht),
        '▀'.with_style(sheet.voltorb_wht),
        '▀'.with_style(sheet.voltorb_wht),
        Texel::empty(),
      ],
      (1, 0) => [
        Texel::empty(),
        b::Char::right_half(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
        Texel::empty(),
      ],
      (1, 1) => [
        Texel::empty(),
        Texel::empty(),
        b::Char::vertical(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
        Texel::empty(),
      ],
      (1, 2) => [
        Texel::empty(),
        b::Char::right_half(sheet.number_weight).with_style(sheet.number_style),
        b::Char::up_tee(sheet.number_weight).with_style(sheet.number_style),
        b::Char::left_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (2, 0) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (2, 1) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::lower_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (2, 2) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::left_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (3, 0) => [
        Texel::empty(),
        b::Char::right_half(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (3, 1) => [
        Texel::empty(),
        Texel::empty(),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::left_tee(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (3, 2) => [
        Texel::empty(),
        b::Char::right_half(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::lower_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (4, 0) => [
        Texel::empty(),
        b::Char::down_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
        b::Char::down_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (4, 1) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::cross(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (4, 2) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::up_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (5, 0) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::left_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (5, 1) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (5, 2) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::lower_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (6, 0) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (6, 1) => [
        Texel::empty(),
        b::Char::right_tee(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (6, 2) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::lower_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (7, 0) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (7, 1) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::vertical(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (7, 2) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::up_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (8, 0) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (8, 1) => [
        Texel::empty(),
        b::Char::right_tee(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::left_tee(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (8, 2) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::lower_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (9, 0) => [
        Texel::empty(),
        b::Char::upper_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::upper_right(sheet.number_weight)
          .with_style(sheet.number_style),
        Texel::empty(),
      ],
      (9, 1) => [
        Texel::empty(),
        b::Char::lower_left(sheet.number_weight).with_style(sheet.number_style),
        b::Char::horizontal(sheet.number_weight).with_style(sheet.number_style),
        b::Char::left_tee(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      (9, 2) => [
        Texel::empty(),
        Texel::empty(),
        Texel::empty(),
        b::Char::up_half(sheet.number_weight).with_style(sheet.number_style),
        Texel::empty(),
      ],
      x => panic!("bad card data: {x:?}"),
    };

    line.copy_from_slice(&art);
  }
}
