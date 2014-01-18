extern mod extra;
use extra::time;
use std::libc::c_int;

use graphics::Display;
use piece_getter::{PieceGetter, new};
use pieces::{Block, Piece, Black};
use set_blocks::SetBlocks;

mod terminal_control {
  use std::libc::c_int;
  
  struct termios {
    c_iflag: c_int,      // input flags
    c_oflag: c_int,      // output flags
    c_cflag: c_int,      // control flags
    c_lflag: c_int,      // local flags
    c_cc:    [u8, ..32], // control characters
    
    // on my machine's C compiler and environment:
    // -- sizeof(int) == 4
    // -- sizeof(struct termios) == 60
    // In this struct's definition so far, 16 bytes of flags + 32 control char bytes equals 48
    // Adding in the extra 12 bytes to match C struct's size
    padding: [u8, ..12]
  }

  extern {
    fn tcgetattr(filedes: c_int, termptr: *mut termios) -> c_int;
    fn tcsetattr(filedes: c_int, opt: c_int, termptr: *termios) -> c_int;
    fn cfmakeraw(termptr: *mut termios);
  }

  fn get_terminal_attr() -> (termios, c_int) {
    unsafe {
      let mut ios = termios {
	c_iflag: 0,
	c_oflag: 0,
	c_cflag: 0,
	c_lflag: 0,
	c_cc:    [0, ..32],
	padding: [0, ..12]
      };
      // first parameter is file descriptor number, 0 ==> standard input
      let err = tcgetattr(0, &mut ios);
      return (ios, err);
    }
  }

  fn make_raw(ios: &termios) -> termios {
    unsafe {
      let mut ios = *ios;
      cfmakeraw(&mut ios);
      return ios;
    }
  }

  fn set_terminal_attr(ios: &termios) -> c_int {
    unsafe {
      // first paramter is file descriptor number, 0 ==> standard input
      // second paramter is when to set, 0 ==> now
      return tcsetattr(0, 0, ios);
    }
  }

  pub struct TerminalRestorer {
    ios: termios
  }

  impl TerminalRestorer {
    pub fn restore(&self) {
      set_terminal_attr(&self.ios);
    }
  }

  pub fn set_terminal_raw_mode() -> TerminalRestorer {
    let (original_ios, err) = get_terminal_attr();
    if err != 0 {
      fail!("failed to get terminal settings");
    }
    
    let raw_ios = make_raw(&original_ios);
    let err = set_terminal_attr(&raw_ios);
    if err != 0 {
      fail!("failed to switch terminal to raw mode");
    }
    
    TerminalRestorer {
      ios: original_ios
    }
  }
  
  // debugging/testing purposes
  /*
  pub fn print_terminal_settings() {
    let (ios, _) = get_terminal_attr();
    println!("iflag = {}", ios.c_iflag);
    println!("oflag = {}", ios.c_oflag);
    println!("cflag = {}", ios.c_cflag);
    println!("lflag = {}", ios.c_lflag);
    println!("control characters:");
    for c in ios.c_cc.iter() {
      println!("{}", *c);
    }
    println("unknown:");
    for a in ios.padding.iter() {
      println!("{}", *a);
    }  
  }
  */
}

mod input_reader {
  use std::libc::{c_int, c_short, c_long};
  use std::cast::transmute;
  
  pub enum PollResult {
    PollReady,
    PollTimeout,
  }
  
  pub enum ReadResult {
    Up, Down, Right, Left, Other
  }
  
  struct pollfd {
    fd:      c_int,
    events:  c_short,
    revents: c_short
  }

  extern {
    fn poll(fds: *pollfd, nfds: c_long, timeout: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut u8, nbyte: u64) -> i64;
  }

  pub fn poll_stdin(timeoutMillis: c_int) -> PollResult {
    unsafe {
      let pfd = pollfd {
	fd:      0, // standard input file descriptor number
	events:  1, // POLLIN event
	revents: 0  // kernel modifies this field when calling poll()
      };
      let pr = poll(&pfd, 1, timeoutMillis);
      if pr > 0 {
	return PollReady
      } else if pr == 0 {
	return PollTimeout;
      } else {
	fail!("error polling standard input");
      }
    }
  }
  
  pub fn read_stdin() -> ReadResult {
    unsafe {
      // reading bytes into storage for an unsigned integer for easy comparison of
      // input byte sequence (we only care about arrow keys) to integer constants
      //
      // at least for Konsole, pressing Up, Down, Right, or Left on the keyboard sends 3 bytes:
      // 0x1B (escape)
      // 0x5B [
      // 0x41, 0x42, 0x43, or 0x44 (A, B, C, or D)
      //
      // note the case where we read less than all 3 bytes from the single read call is not handled, and considered "Other"
      //
      // for example, 0x1B 0x5B, 0x44 is sent when Left is pressed
      // 
      // the integer constants to compare these sequences to are "backwards" due to Intel's least significant byte order
      // example above is least significant byte order representation of 0x445B1B
      
      let mut buf = 0u64;
      let bufAddr: *mut u8 = transmute(&mut buf);
      
      // first parameter is file descriptor number, 0 ==> standard input
      let numRead = read(0, bufAddr, 8);
      if numRead < 0 {
	fail!("error reading standard input");
      }
      match buf {
	0x415B1B => Up,
	0x425B1B => Down,
	0x435B1B => Right,
	0x445B1B => Left,
	_        => Other
      }
    }
  }  
}

mod graphics {
  use std::io::stdio::flush;
  use pieces::{Block, Piece, O, S};
  
  fn csi() {
    print!("{}[", '\x1B');
  }

  fn clear_terminal() {
    csi();
    print("2J");
  }

  fn reset_graphics() {
    csi();
    print("0m");
  }

  fn move_cursor(row: i8, column: i8) {
    csi();
    print!("{};{}H", row, column);
  }
  
  fn set_background_color(offset: u8) {
    csi();
    print!("{}m", 40 + offset);
  }
  
  fn print_borders(rows: i8, cols: i8, rowOffset: i8, columnOffset: i8) {
    reset_graphics();

    // sides
    let mut row = 1;
    while row <= rows + 1 {
      move_cursor(row + rowOffset, 1 + columnOffset);
      print("<!");
      move_cursor(row + rowOffset, 3 + cols + columnOffset);
      print("!>");
      row += 1;
    }
    
    // bottom
    move_cursor(rows + rowOffset + 1, 3 + columnOffset);
    let mut col = 1;
    while col <= cols {
      print("=");
      col += 1;
    }
    move_cursor(rows + rowOffset + 2, 3 + columnOffset);
    col = 1;
    while col <= cols - 1 {
      print("\\/");
      col += 2;
    }
  }
  
  pub trait Display {
    fn init(&self);
    fn print_block(&self, block: Block);
    fn print_next_piece(&self, piece: &Piece);
    fn close(&self);
    fn flush(&self);
  }

  pub struct StandardDisplay;

  // terminal level row/column offsets for everything (Blocks, borders, ...)
  static stdRowOffset: i8 = 2i8;
  static stdColumnOffset: i8 = 3i8;
  
  // terminal level number of columns a left/right border takes
  static stdBorderColumns: i8 = 2i8;
  
  impl Display for StandardDisplay {
    fn print_block(&self, block: Block) {
      if block.row < 1 || block.column < 1 {
	return;
      }
      move_cursor(block.row + stdRowOffset, 2 * block.column + stdBorderColumns - 1 + stdColumnOffset);
      set_background_color(block.color as u8);
      print("  ");
    }
    
    fn print_next_piece(&self, piece: &Piece) {
      let colOffset = match piece.ty {
	O | S => 12,
	_     => 13
      };
      for block in piece.blocks.iter() {
	move_cursor(5 + block.row + stdRowOffset, 2 * (colOffset + block.column) + stdBorderColumns - 1 + stdColumnOffset);
	set_background_color(block.color as u8);
	print("  ");
      }

    }
    
    fn init(&self) {
      clear_terminal();
      print_borders(20, 20, stdRowOffset, stdColumnOffset);
      
      move_cursor(5 + stdRowOffset, 2 * 14 + stdBorderColumns - 1 + stdColumnOffset);
      print("Next:");
      
      flush();
    }
    
    fn close(&self) {
      reset_graphics();
    }
    
    fn flush(&self) {
      flush();
    }
  }
  /*
  pub struct DoubleDisplay;
  
  static dblRowOffset: i8 = 2i8;
  static dblColumnOffset: i8 = 30i8;
  static dblBorderColumns: i8 = 2i8;
  
  impl Display for DoubleDisplay {
    fn print_block(&self, block: Block) {
       if block.row < 1 || block.column < 1 {
	return;
      }
      move_cursor(2 * block.row + dblRowOffset, 4 * block.column - 3 + dblBorderColumns + dblColumnOffset);
      set_background_color(block.color as u8);
      print("    ");
      move_cursor(2 * block.row - 1 + dblRowOffset, 4 * block.column - 3 + dblBorderColumns + dblColumnOffset);
      print("    ");
    }
    
    fn init(&self) {
      clear_display();
      print_borders(40, 40, dblRowOffset, dblColumnOffset);
    }
    
    fn close(&self) {
      reset_graphics();
    }
  }
  */
}

mod pieces {
  #[deriving(Eq)]
  pub enum Color {
    Black = 0, Red, Green, Yellow, Blue, Magenta, Cyan, White
  }

  pub struct Block {
    row:    i8,
    column: i8,
    color:  Color
  }

  #[deriving(Clone)]
  pub enum PieceType {
    I = 0, J, L, O, S, T, Z
  }

  pub struct Piece {
    ty:     PieceType,
    rotate: u8,
    blocks: [Block, ..4]
  }


  /*
  Pieces Table:

    | | | | |     | |0| | |     | | | | |     | |3| | |
    | | | | |     | |1| | |     | | | | |     | |2| | |
  I | | | | | --> | |2| | | --> | | | | | --> | |1| | |
    |0|1|2|3|     | |3| | |     |3|2|1|0|     | |0| | |

    | | | | |     | | | | |     | | | | |     | | | | |
    | | | | |     | |1|0| |     | | | | |     | |3| | |
  J |0| | | | --> | |2| | | --> |3|2|1| | --> | |2| | |
    |1|2|3| |     | |3| | |     | | |0| |     |0|1| | |

    | | | | |     | | | | |     | | | | |     | | | | |
    | | | | |     |0| | | |     | | | | |     |3|2| | |
  L | | |3| | --> |1| | | | --> |2|1|0| | --> | |1| | |
    |0|1|2| |     |2|3| | |     |3| | | |     | |0| | |

    | | | | |     | | | | |     | | | | |     | | | | |
    | | | | |     | | | | |     | | | | |     | | | | |
  O |0|1| | | --> |0|1| | | --> |0|1| | | --> |0|1| | |
    |2|3| | |     |2|3| | |     |2|3| | |     |2|3| | |

    | | | | |     | | | | |     | | | | |     | | | | |
    | | | | |     |0| | | |     | | | | |     |3| | | |
  S | |2|3| | --> |1|2| | | --> | |1|0| | --> |2|1| | |
    |0|1| | |     | |3| | |     |3|2| | |     | |0| | |

    | | | | |     | | | | |     | | | | |     | | | | |
    | | | | |     |0| | | |     | | | | |     | |2| | |
  T | |3| | | --> |1|3| | | --> |2|1|0| | --> |3|1| | |
    |0|1|2| |     |2| | | |     | |3| | |     | |0| | |

    | | | | |     | | | | |     | | | | |     | | | | |
    | | | | |     | |0| | |     | | | | |     | |3| | |
  Z |0|1| | | --> |2|1| | | --> |3|2| | | --> |1|2| | |
    | |2|3| |     |3| | | |     | |1|0| |     |0| | | |
  */

  static pieceInitial: [Piece, ..7] = 
  [
    Piece{ty:     I,
	  rotate: 0,
	  blocks: [Block{row: 0, column: 4, color: Cyan},
		   Block{row: 0, column: 5, color: Cyan},
		   Block{row: 0, column: 6, color: Cyan},
		   Block{row: 0, column: 7, color: Cyan}]},
    
    Piece{ty:     J,
	  rotate: 0,
	  blocks: [Block{row: -1, column: 4, color: Blue},
		   Block{row:  0, column: 4, color: Blue},
		   Block{row:  0, column: 5, color: Blue},
		   Block{row:  0, column: 6, color: Blue}]},
    
    Piece{ty:     L,
	  rotate: 0,
	  blocks: [Block{row:  0, column: 4, color: White},
		   Block{row:  0, column: 5, color: White},
		   Block{row:  0, column: 6, color: White},
		   Block{row: -1, column: 6, color: White}]},

    Piece{ty:     O,
	  rotate: 0,
	  blocks: [Block{row: -1, column: 5, color: Yellow},
		   Block{row: -1, column: 6, color: Yellow},
		   Block{row:  0, column: 5, color: Yellow},
		   Block{row:  0, column: 6, color: Yellow}]},
    
    Piece{ty:     S,
	  rotate: 0,
	  blocks: [Block{row:  0, column: 5, color: Green},
		   Block{row:  0, column: 6, color: Green},
		   Block{row: -1, column: 6, color: Green},
		   Block{row: -1, column: 7, color: Green}]},
    
    Piece{ty:     T,
	  rotate: 0,
	  blocks: [Block{row:  0, column: 4, color: Magenta},
		   Block{row:  0, column: 5, color: Magenta},
		   Block{row:  0, column: 6, color: Magenta},
		   Block{row: -1, column: 5, color: Magenta}]},

    Piece{ty:     Z,
	  rotate: 0,
	  blocks: [Block{row: -1, column: 4, color: Red},
		   Block{row: -1, column: 5, color: Red},
		   Block{row:  0, column: 5, color: Red},
		   Block{row:  0, column: 6, color: Red}]}
  ];

  pub fn new(ty: PieceType) -> Piece {
    pieceInitial[ty as int]
  }
  
  static pieceRotate: [[[(i8, i8), ..4], ..4], ..7] =
  [
    // I
    [[(-3,1),(-2,0),(-1,-1),(0,-2)], [(3,2),(2,1),(1,0),(0,-1)], [(0,-2),(-1,-1),(-2,0),(-3,1)], [(0,-1),(1,0),(2,1),(3,2)]],

    // J
    [[(-1,2),(-2,1),(-1,0),(0,-1)], [(2,0),(1,1),(0,0),(-1,-1)], [(0,-2),(1,-1),(0,0),(-1,1)], [(-1,0),(0,-1),(1,0),(2,1)]],

    // L
    [[(-2,0),(-1,-1),(0,-2),(1,-1)], [(1,2),(0,1),(-1,0),(0,-1)], [(1,-1),(0,0),(-1,1),(-2,0)], [(0,-1),(1,0),(2,1),(1,2)]],

    // O
    [[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]],

    // S
    [[(-2,0),(-1,-1),(0,0),(1,-1)], [(1,2),(0,1),(1,0),(0,-1)], [(1,-1),(0,0),(-1,-1),(-2,0)], [(0,-1),(1,0),(0,1),(1,2)]],

    // T
    [[(-2,0),(-1,-1),(0,-2),(0,0)], [(1,2),(0,1),(-1,0),(1,0)], [(1,-1),(0,0),(-1,1),(-1,-1)], [(0,-1),(1,0),(2,1),(0,1)]],

    // Z
    [[(-1,1),(0,0),(-1,-1),(0,-2)], [(2,1),(1,0),(0,1),(-1,0)], [(0,-2),(-1,-1),(0,0),(-1,1)], [(-1,0),(0,1),(1,0),(2,1)]]
  ];

  trait Offset {
    fn row(self) -> i8;
    fn col(self) -> i8;
  }

  impl Offset for (i8, i8) {
    fn row(self) -> i8 {
      let (row, _) = self;
      row
    }
    
    fn col(self) -> i8 {
      let (_, col) = self;
      col
    }
  }
  
  fn transform_blocks(clockwise: bool, blocks: &[Block, ..4], transform: [(i8, i8), ..4]) -> [Block, ..4] {
    let s = if clockwise { 1i8 } else { -1i8 };
    [
      Block{row:    blocks[0].row    + s * transform[0].row(),
	    column: blocks[0].column + s * transform[0].col(),
	    color:  blocks[0].color},
    
      Block{row:    blocks[1].row    + s * transform[1].row(),
	    column: blocks[1].column + s * transform[1].col(),
	    color:  blocks[1].color},
    
      Block{row:    blocks[2].row    + s * transform[2].row(),
	    column: blocks[2].column + s * transform[2].col(),
	    color:  blocks[2].color},
    
      Block{row:    blocks[3].row    + s * transform[3].row(),
	    column: blocks[3].column + s * transform[3].col(),
	    color:  blocks[3].color}
    ]
  }

  pub fn rotate_clockwise(piece: &Piece) -> Piece {
    Piece {
      ty:     piece.ty,
      rotate: (piece.rotate + 1) % 4,
      blocks: transform_blocks(true, &piece.blocks, pieceRotate[piece.ty as int][piece.rotate])
    }
  }

  pub fn rotate_counter_clockwise(piece: &Piece) -> Piece {
    Piece {
      ty:     piece.ty,
      rotate: (piece.rotate + 3) % 4,
      blocks: transform_blocks(false, &piece.blocks, pieceRotate[piece.ty as int][(piece.rotate + 3) % 4])
    }
  }
  
  pub fn translate(piece: &Piece, rowOffset: i8, columnOffset: i8) -> Piece {
    Piece {
      ty:     piece.ty,
      rotate: piece.rotate,
      blocks: [Block{row:    piece.blocks[0].row    + rowOffset,
                     column: piece.blocks[0].column + columnOffset,
                     color:  piece.blocks[0].color},
                     
               Block{row:    piece.blocks[1].row    + rowOffset,
                     column: piece.blocks[1].column + columnOffset,
                     color:  piece.blocks[1].color},
                     
               Block{row:    piece.blocks[2].row    + rowOffset,
                     column: piece.blocks[2].column + columnOffset,
                     color:  piece.blocks[2].color},
               
               Block{row:    piece.blocks[3].row + rowOffset,
                     column: piece.blocks[3].column + columnOffset,
                     color:  piece.blocks[3].color}]
    }
  }
}

mod set_blocks {
  use pieces::Block;
  
  pub trait SetBlocks {
    fn has_block(&self, row: i8, col: i8) -> bool;  
    fn get(&self, row: i8, col: i8) -> Option<Block>;
    fn remove(&mut self, row: i8, col: i8);
    fn set(&mut self, block: Block);
  }

  #[inline]
  fn index(row: i8, col: i8) -> int {
    return 10 * ((row as int) - 1) + (col as int) - 1;
  }

  impl SetBlocks for [Option<Block>, ..200] {
    fn has_block(&self, row: i8, col: i8) -> bool {
      if row < 1 || row > 20 || col < 1 || col > 10 {
	return false;
      }
      return self[index(row, col)].is_some();
    }
    
    fn get(&self, row: i8, col: i8) -> Option<Block> {
      return self[index(row, col)];
    }
    
    fn remove(&mut self, row: i8, col: i8) {
      self[index(row, col)] = None;
    }
    
    fn set(&mut self, block: Block) {
      if block.row < 1 || block.row > 20 || block.column < 1 || block.column > 10 {
	fail!("can't add out of bounds block to set blocks");
      }
      self[index(block.row, block.column)] = Some(block);
    }
  }
}

mod piece_getter {
  use pieces;
  use pieces::{Piece, I, J, L, O, S, T, Z};
  use std::rand::Rng;
  use std::rand::os::OSRng;

  pub trait PieceGetter {
    fn next_piece(&mut self) -> Piece;
  }
  
  pub fn new() -> ~PieceGetter {
    return ~RandomPieceGetter{rng: OSRng::new()} as ~PieceGetter;
  }
  
  struct RandomPieceGetter {
    rng: OSRng
  }
  
  impl PieceGetter for RandomPieceGetter {
    fn next_piece(&mut self) -> Piece {
      let pieceType = self.rng.choose(&[I, J, L, O, S, T, Z]);
      return pieces::new(pieceType);
    }
  }
}

trait GameHandler {
  fn handle_step(&mut self) -> Option<c_int>;
  fn handle_input(&mut self, input: input_reader::ReadResult);
}

enum State {
  Fall = 0, Clear, GameOver
}

struct TetrisGame<'a> {
  display:     &'a Display,
  pieceGetter: &'a mut PieceGetter,
  level:       int,
  score:       int,
  bonus:       int,
  bonusDrop:   int,
  state:       State,
  piece:       Piece,
  nextPiece:   Piece,
  setBlocks:   [Option<Block>, ..200]
}

// 
static bonus_drop_reset: int = 2;

// 
static levels : [(c_int, int, int), ..1] = [(1000, 0, 5)];


trait Level {
  fn time(self) -> c_int;
  fn score(self) -> int;
  fn count(self) -> int;
}

impl Level for (c_int, int, int) {
  fn time(self) -> c_int {
    let (time, _, _) = self;
    time
  }
  fn score(self) -> int {
    let (_, score, _) = self;
    score
  }
  fn count(self) -> int {
    let (_, _, count) = self;
    count
  }
}

fn get_level_score(level: int) -> int {
  levels[level - 1].score()
}

fn get_base_score(setRows: int) -> int {
  10 * (1 << (setRows - 1))
}

fn get_level_time(level: int) -> c_int {
  levels[level - 1].time()
}

impl<'a> TetrisGame<'a> {
  
  // TODO: fn new(piece getter impl)
  
  fn collides_with_set_blocks(&self, piece: &Piece) -> bool {
    for block in piece.blocks.iter() {
      if self.setBlocks.has_block(block.row, block.column) {
	return true;
      }
    }
    return false;
  }
  
  fn in_bounds_bottom_row(piece: &Piece) -> bool {
    for block in piece.blocks.iter() {
      if block.row > 20 {
	return false;
      }
    }
    return true;
  }
  
  fn in_bounds_cols(piece: &Piece) -> bool {
    for block in piece.blocks.iter() {
      if block.column < 1 || block.column > 10 {
	return false;
      }
    }
    return true;
  }
  
  fn all_in_bounds(piece: &Piece) -> bool {
    for block in piece.blocks.iter() {
      if block.row < 1 || block.row > 20 || block.column < 1 || block.column > 10 {
	return false;
      }
    }
    return true;
  }
  
  fn can_move_rows(&self, piece: &Piece, rowOffset: i8) -> bool {
    let moved =  pieces::translate(piece, rowOffset, 0);
    return TetrisGame::in_bounds_bottom_row(&moved) && !self.collides_with_set_blocks(&moved);
  }

  fn is_row_set(&self, row: i8) -> bool {
    let mut col = 1;
    while self.setBlocks.has_block(row, col) {
      col += 1;
    }
    return col == 11i8;
  }
  
  fn set_row_count(&self) -> int {
    let mut count = 0;
    for row in range(1, 21i8) {
      if self.is_row_set(row) {
	count += 1;
      }
    }
    return count;
  }
  
  fn erase_block(&self, row: i8, col: i8) {
    self.display.print_block(Block{row: row, column: col, color: Black});
  }
  
  fn erase_piece(&self) {
    for block in self.piece.blocks.iter() {
       self.erase_block(block.row, block.column);
    }
  }
  
  fn erase_row(&self, row: i8) {
    for col in range(1, 11i8) {
      self.erase_block(row, col);
    }
  }
  
  fn erase_set_rows(&self) {
    for row in range(1, 21i8) {
      if self.is_row_set(row) {
	self.erase_row(row);
      }
    }
    self.display.flush();
  }
  
  fn erase_all_set_blocks(&self) {
    for row in range(1, 21i8) {
      for col in range(1, 11i8) {
	match self.setBlocks.get(row, col) {
	  None                                => (),
	  Some(block) if block.color != Black => self.erase_block(row, col),
	  _                                   => ()
	}
      }
    }
  }
  
  fn erase_next_piece(&self) {
    let erase = Piece {
                  ty:     self.nextPiece.ty,
                  rotate: self.nextPiece.rotate,
                  blocks: [Block{row:    self.nextPiece.blocks[0].row,
                                 column: self.nextPiece.blocks[0].column,
                                 color:  Black},
                           
                           Block{row:    self.nextPiece.blocks[1].row,
                                 column: self.nextPiece.blocks[1].column,
                                 color:  Black},
                           
                           Block{row:    self.nextPiece.blocks[2].row,
                                 column: self.nextPiece.blocks[2].column,
                                 color:  Black},
                           
                           Block{row:    self.nextPiece.blocks[3].row,
                                 column: self.nextPiece.blocks[3].column,
                                 color:  Black}]
    };
    self.display.print_next_piece(&erase);
  }
  
  fn print_piece(&self) {
    for block in self.piece.blocks.iter() {
      self.display.print_block(*block);
    }
    self.display.flush();
  }

  fn print_set_blocks(&self) {
    for row in range(1, 21i8) {
      for col in range(1, 11i8) {
	match self.setBlocks.get(row, col) {
	  None        => (),
	  Some(block) => self.display.print_block(block),
	}
      }
    }
    self.display.flush();
  }
  
  fn set_piece(&mut self) {
    for block in self.piece.blocks.iter() {
      self.setBlocks.set(*block);
    }
  }
  
  fn clear_row(&mut self, row: i8) {
    for col in range(1, 11i8) {
      let mut r = row;
      while r >= 2 {
	match self.setBlocks.get(r - 1, col) {
	  None        => self.setBlocks.remove(r, col),
	  Some(block) => self.setBlocks.set(Block{row: r, column: col, color: block.color})
	}
	r -= 1;
      }
    }
    for col in range(1, 11i8) {
      self.setBlocks.remove(1, col);
    }
  }
  
  fn clear_set_rows(&mut self) {
    let mut row = 20;
    loop {
      while row >= 1 && !self.is_row_set(row) {
	row -= 1;
      }
      if row == 0 {
	break;
      } else {
	self.clear_row(row);      
      }
    }
  }
  
  fn update_piece(&mut self, next: &Piece) {
    self.erase_piece();
    
    self.piece = *next;
    
    self.print_piece();
  }
  
  fn go_to_next_piece(&mut self) {
      self.set_piece();
      
      self.erase_next_piece();
      
      self.piece = self.nextPiece;
      self.nextPiece = self.pieceGetter.next_piece();
      
      self.display.print_next_piece(&self.nextPiece);    
  }
  
  fn step_fall(&mut self) -> Option<c_int> {
    match self.can_move_rows(&self.piece, 1) {
      true  => {
	let translated = pieces::translate(&self.piece, 1, 0);
	self.update_piece(&translated);
	Some(get_level_time(self.level))
      }
      false => {
	if !TetrisGame::all_in_bounds(&self.piece) {
	  self.state = GameOver;
	  return Some(500);
	}
	
	self.go_to_next_piece();
	
	// TODO: possibly refactor out scoring calculations into their own method
	
	let setRows = self.set_row_count();
	if setRows > 0 {
	  self.score += (get_base_score(setRows) + get_level_score(self.level)) * self.bonus;
	  if self.bonus == 1 {
	    self.bonus = 2 * setRows;
	  } else {
	    self.bonus += 2 * setRows;
	  }
	  self.bonusDrop = bonus_drop_reset;
	  
	  self.erase_set_rows();
	  self.state = Clear;
	} else {
	  // TODO: update number of dropped pieces, bump level if needed
	  if self.bonus > 1 {
	    self.bonusDrop -= 1;
	    if self.bonusDrop == 0 {
	      self.bonus -= 1;
	      self.bonusDrop = bonus_drop_reset;
	    }	  
	  }
	}
	Some(1000)
      }
    }
  }
  
  fn step_clear(&mut self) -> Option<c_int> {
    self.erase_all_set_blocks();
    
    self.clear_set_rows();
    
    self.print_set_blocks();
    
    self.state = Fall;
    
    Some(1000)
  }
  
  fn step_game_over(&mut self) -> Option<c_int> {
    None
  }
  
  fn rotate(&mut self, clockwise: bool) {
    let rotated = if clockwise {
      pieces::rotate_clockwise(&self.piece)
    } else {
      pieces::rotate_counter_clockwise(&self.piece)
    };
    
    if !TetrisGame::in_bounds_cols(&rotated) || self.collides_with_set_blocks(&rotated) {
      return;
    }
    
    self.update_piece(&rotated);
  }
  
  fn quick_drop(&mut self) {
    if !self.can_move_rows(&self.piece, 1) {
      return;
    }
    
    let mut translated = pieces::translate(&self.piece, 1, 0);
    while self.can_move_rows(&translated, 1) {
      translated = pieces::translate(&translated, 1, 0);
    }
    
    self.update_piece(&translated);
  }
  
  fn translate_cols(&mut self, columnOffset: i8) {
    let translated = pieces::translate(&self.piece, 0, columnOffset);
    
    if !TetrisGame::in_bounds_cols(&translated) || self.collides_with_set_blocks(&translated) {
      return;
    }
    
    self.update_piece(&translated);
  }
}

impl<'a> GameHandler for TetrisGame<'a> {
  fn handle_step(&mut self) -> Option<c_int> {    
    match self.state {
      Fall     => self.step_fall(),
      Clear    => self.step_clear(),
      GameOver => self.step_game_over()
    }
  }
  
  fn handle_input(&mut self, input: input_reader::ReadResult) {
    use input_reader::{Up, Down, Right, Left};
    match input {
      Up    => self.rotate(true),
      Down  => self.quick_drop(),
      Right => self.translate_cols(1),
      Left  => self.translate_cols(-1),
      _     => fail!("unknown direction")
    }
  }
}

fn main_loop<T: GameHandler>(handler: &mut T) {
  use input_reader::{poll_stdin, read_stdin, Other, PollReady, PollTimeout};
  
  // milliseconds between piece drop steps
  let mut stepTimeMs: c_int = 1000;
  
  // milliseconds for poll timeout
  let mut pollTimeMs = stepTimeMs;
  
  // nanoseconds since the last drop step
  let mut sinceLastStepNs = 0u64;
  
  loop {
    let t = time::precise_time_ns();
    match poll_stdin(pollTimeMs) {
      PollReady   => {
	match read_stdin() {
	  Other => { break; }
	  input => {
	    sinceLastStepNs += time::precise_time_ns() - t;
	    pollTimeMs = stepTimeMs - ((sinceLastStepNs / 1000000) as c_int);
	    handler.handle_input(input);
	  }
	}
      }
      PollTimeout => {
	match handler.handle_step() {
	  None                 => { break; }
	  Some(nextStepTimeMs) => {
	    stepTimeMs = nextStepTimeMs;
	    pollTimeMs = nextStepTimeMs;
	    sinceLastStepNs = 0;
	  }
	}
      }
    }
  }
}

fn main() {
  let restorer = terminal_control::set_terminal_raw_mode();
  
  // TODO: print the instructions, wait for the user to press Enter to start the game
  
  // TODO: ask the user if they want standard or double size
  let display = graphics::StandardDisplay;
  display.init();
  
  let mut pieceGetter = piece_getter::new();
  let firstPiece = pieceGetter.next_piece();
  let secondPiece = pieceGetter.next_piece();

  display.print_next_piece(&secondPiece);
  
  let mut game = TetrisGame{display:     &display,
                            pieceGetter: pieceGetter,
                            level:       1,
                            score:       0,
                            bonus:       1,
                            bonusDrop:   bonus_drop_reset,
                            state:       Fall,
                            piece:       firstPiece,
                            nextPiece:   secondPiece,
                            setBlocks:   [None, ..200]};

  main_loop(&mut game);
  
  display.close();
  restorer.restore();

}
