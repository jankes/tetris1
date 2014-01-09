extern mod extra;
use extra::time;
use std::libc::c_int;

use graphics::Display;

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

mod terminal_control {
  use std::libc::c_int;
  
  struct termios {
    c_iflag: c_int,     // input flags
    c_oflag: c_int,     // output flags
    c_cflag: c_int,     // control flags
    c_lflag: c_int,     // local flags
    c_cc: [u8, ..32],   // control characters
    
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
	c_cc: [0, ..32],
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

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

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
    fd: c_int,
    events: c_short,
    revents: c_short
  }

  extern {
    fn poll(fds: *pollfd, nfds: c_long, timeout: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut u8, nbyte: u64) -> i64;
  }

  pub fn poll_stdin(timeoutMillis: c_int) -> PollResult {
    unsafe {
      let pfd = pollfd {
	fd: 0,     // standard input file descriptor number
	events: 1, // POLLIN event
	revents: 0 // kernel modifies this field when calling poll()
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

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

mod graphics {
  use std::io::stdio::flush;
  use super::Block;
  
  fn csi() {
    print!("{}[", '\x1B');
  }

  fn clear_display() {
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
    
    fn init(&self) {
      clear_display();
      print_borders(20, 20, stdRowOffset, stdColumnOffset);
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

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

enum Color {
  Black = 0, Red, Green, Yellow, Blue, Magenta, Cyan, White
}

struct Block {
  row: i8,
  column: i8,
  color: Color
}

enum PieceType {
  I = 0, J, L, O, S, T, Z
}

struct Piece {
  ty: PieceType,
  rotate: u8,
  blocks: [Block, ..4]
}

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

static pieceInitial: [Piece, ..2] = 
[
  Piece{ty:     I,
        rotate: 0,
        blocks: [Block{row: 0, column: 4, color: Cyan},
                 Block{row: 0, column: 5, color: Cyan},
                 Block{row: 0, column: 6, color: Cyan},
                 Block{row: 0, column: 7, color: Cyan}]},
  
  Piece{ty:     J,
        rotate: 0,
        blocks: [Block{row: -1, column: 3, color: Blue},
                 Block{row: 0, column: 3, color: Blue},
                 Block{row: 0, column: 4, color: Blue},
                 Block{row: 0, column: 5, color: Blue}]},
/*
   
   // L
   [Block{row: -1, column: 4, color: Cyan},
    Block{row: -1, column: 5, color: Cyan},
    Block{row: -1, column: 6, color: Cyan},
    Block{row: -1, column: 7, color: Cyan}],
   
   // O
   [Block{row: -1, column: 4, color: Cyan},
    Block{row: -1, column: 5, color: Cyan},
    Block{row: -1, column: 6, color: Cyan},
    Block{row: -1, column: 7, color: Cyan}],
   
   // S
   [Block{row: -1, column: 4, color: Cyan},
    Block{row: -1, column: 5, color: Cyan},
    Block{row: -1, column: 6, color: Cyan},
    Block{row: -1, column: 7, color: Cyan}],
    
   // T
   [Block{row: -1, column: 4, color: Cyan},
    Block{row: -1, column: 5, color: Cyan},
    Block{row: -1, column: 6, color: Cyan},
    Block{row: -1, column: 7, color: Cyan}],
    
   // Z
   [Block{row: -1, column: 4, color: Cyan},
    Block{row: -1, column: 5, color: Cyan},
    Block{row: -1, column: 6, color: Cyan},
    Block{row: -1, column: 7, color: Cyan}],
*/
];

static pieceRotate: [[[(i8, i8), ..4], ..4], ..7] =
[
// I
[[(-3,1),(-2,0),(-1,-1),(0,-2)], [(3,2),(2,1),(1,0),(0,-1)], [(0,-2),(-1,-1),(-2,0),(-3,1)], [(0,-1),(1,0),(2,1),(3,2)]],

// J
[[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]],

// L
[[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]],

// O
[[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]],

// S
[[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]],

// T
[[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]],

// Z
[[(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)], [(0,0),(0,0),(0,0),(0,0)]]
];

fn transform_blocks(clockwise: bool, blocks: &[Block, ..4], transform: [(i8, i8), ..4]) -> [Block, ..4] {
  let s = if clockwise { 1i8 } else { -1i8 };
  [
  Block{row:    blocks[0].row    + s * transform[0].row(),
        column: blocks[0].column + s * transform[0].col(),
        color:  blocks[0].color},
  
  Block{row:    blocks[1].row    + s * transform[1].row(),
        column: blocks[1].column + s * transform[1].col(),
        color: blocks[1].color},
  
  Block{row:    blocks[2].row    + s * transform[2].row(),
        column: blocks[2].column + s * transform[2].col(),
        color:  blocks[2].color},
  
  Block{row:    blocks[3].row    + s * transform[3].row(),
        column: blocks[3].column + s * transform[3].col(),
        color:  blocks[3].color}
  ]
}

fn rotate_clockwise(piece: &Piece) -> Piece {
  Piece {
    ty: piece.ty,
    rotate: (piece.rotate + 1) % 4,
    blocks: transform_blocks(true, &piece.blocks, pieceRotate[piece.ty as int][piece.rotate])
  }
}

fn rotate_counter_clockwise(piece: &Piece) -> Piece {
  Piece {
    ty: piece.ty,
    rotate: (piece.rotate + 3) % 4,
    blocks: transform_blocks(false, &piece.blocks, pieceRotate[piece.ty as int][(piece.rotate + 3) % 4])
  }
}


trait GameHandler {
  fn init(&mut self);
  fn handle_step(&mut self);
  fn handle_input(&mut self, input: input_reader::ReadResult);
}

struct OnePieceGame<'a> {
  display: &'a Display,
  piece: Piece
}

impl<'a> OnePieceGame<'a> {
  fn erase(&self) {
    for block in self.piece.blocks.iter() {
      self.display.print_block(Block{row: block.row, column: block.column, color: Black}); 
    }
  }
  
  fn print(&self) {
    for block in self.piece.blocks.iter() {
      self.display.print_block(*block);
    }
    self.display.flush();
  }
  
  fn translate(&mut self, rowOffset: i8, columnOffset: i8) {
    self.erase();
    
    for block in self.piece.blocks.mut_iter() {
      block.row += rowOffset;
      block.column += columnOffset;
    }
    
    self.print();
  }
  
  fn rotate(&mut self, clockwise: bool) {
    self.erase();
    
    if clockwise {
      self.piece = rotate_clockwise(&self.piece);
    } else { 
      self.piece = rotate_counter_clockwise(&self.piece);
    }
    
    self.print();
  }
}

impl<'a> GameHandler for OnePieceGame<'a> {
  fn init(&mut self) {
    
  }
  
  fn handle_step(&mut self) {    
    self.translate(1, 0);
  }
  
  fn handle_input(&mut self, input: input_reader::ReadResult) {
    use input_reader::{Up, Down, Right, Left};
    match input {
      Up    => self.rotate(true),
      Down  => self.rotate(false),
      Right => self.translate(0, 1),
      Left  => self.translate(0, -1),
      _     => fail!("unknown direction")
    }
  }
}

fn main_loop<T: GameHandler>(handler: &mut T) {
  use input_reader::{poll_stdin, read_stdin, Other, PollReady, PollTimeout};
  
  // milliseconds between piece drop steps
  let stepTimeMs: c_int = 1000;
  
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
	pollTimeMs = stepTimeMs;
	sinceLastStepNs = 0;
	handler.handle_step();
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
  
  let mut game = OnePieceGame{display: &display as &Display, piece: pieceInitial[I as int]};
  game.init();
  main_loop(&mut game);
  
  display.close();
  restorer.restore();
}
