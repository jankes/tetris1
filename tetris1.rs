extern crate serialize;
extern crate time;

use std::io::{print, println};
use std::os;

mod terminal_control {
  use std::libc::{c_int, c_uint, c_uchar};
  
  // Linux specifc termios structure definition
  //
  // Since we don't actually access any of the fields individually, and instead just
  // pass around termios as a "black box", this will probably work for other platforms
  // as long their struct termios is smaller than Linux's. For example, Mac OS omits the
  // c_line field and only has 20 control characters.
  #[allow(non_camel_case_types)]
  struct termios {
    c_iflag:  c_uint,          // input mode flags
    c_oflag:  c_uint,          // output mode flags
    c_cflag:  c_uint,          // control mode flags
    c_lflag:  c_uint,          // local mode flags
    c_line:   c_uchar,         // line discipline
    c_cc:     [c_uchar, ..32], // control characters
    c_ispeed: c_uint,          // input speed
    c_ospeed: c_uint,          // output speed
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
        c_line: 0,
        c_cc:    [0, ..32],
        c_ispeed: 0,
        c_ospeed: 0
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

  impl Drop for TerminalRestorer {
    fn drop(&mut self) {
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
  
  #[allow(non_camel_case_types)]
  struct pollfd {
    fd:      c_int,
    events:  c_short,
    revents: c_short
  }

  extern {
    fn poll(fds: *mut pollfd, nfds: c_long, timeout: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut u8, nbyte: u64) -> i64;
  }

  pub fn poll_stdin(timeoutMillis: c_int) -> PollResult {
    unsafe {
      let mut pfd = pollfd {
        fd:      0, // standard input file descriptor number
        events:  1, // POLLIN event
        revents: 0  // kernel modifies this field when calling poll()
      };
      let pr = poll(&mut pfd, 1, timeoutMillis);
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
      // Reading bytes into storage for an unsigned integer for easy comparison of
      // input byte sequence (we only care about arrow keys) to integer constants
      //
      // At least for Konsole, pressing Up, Down, Right, or Left on the keyboard sends 3 bytes:
      // 0x1B (escape)
      // 0x5B [
      // 0x41, 0x42, 0x43, or 0x44 (A, B, C, or D)
      //
      // Note the case where we read less than all 3 bytes from the single read call is not handled,
      // and considered "Other"
      //
      // For example, 0x1B 0x5B, 0x44 is sent when Left is pressed
      // 
      // The integer constants to compare these sequences to are "backwards" due to Intel's least significant
      // byte order, so 0x445B1B is the constant we expect when left is pressed
      
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
  use std::io::stdio;
  use std::io::print;
  use pieces::{Block, Black, Piece, O, S};
  use scoring::Score;
  
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

  fn hide_cursor() {
    csi();
    print("?25l");
  }
  
  fn show_cursor() {
    csi();
    print("?25h");
  }
  
  fn move_cursor(rowCol: (i8, i8)) {
    let (row, col) = rowCol;
    csi();
    print!("{};{}H", row, col);
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
      move_cursor((row + rowOffset, 1 + columnOffset));
      print("<!");
      move_cursor((row + rowOffset, 3 + cols + columnOffset));
      print("!>");
      row += 1;
    }
    
    // bottom
    move_cursor((rows + rowOffset + 1, 3 + columnOffset));
    let mut col = 1;
    while col <= cols {
      print("=");
      col += 1;
    }
    move_cursor((rows + rowOffset + 2, 3 + columnOffset));
    col = 1;
    while col <= cols - 1 {
      print("\\/");
      col += 2;
    }
  }
  
  // convert from game level row and column to terminal row/col
  trait Converter {
    fn to_terminal(&self, row: i8, col: i8) -> (i8, i8);
  }
  
  // game level rows for the information area (displaying score info, next piece)
  static levelRow: i8 = 2;
  static bonusRow: i8 = 4;
  static scoreRow: i8 = 6;
  static nextRow: i8 = 10;
  
  // base game level column for the information area
  // Display implemenations may use an offset from this
  static baseInfoCol: i8 = 14;
  
  fn init<T: Converter>(converter: T,
                        terminalRows: i8,
                        terminalCols: i8,
                        terminalRowOffset: i8,
                        terminalColumnOffset: i8,
                        infoCol: i8) {
      clear_terminal();
      hide_cursor();
      print_borders(terminalRows, terminalCols, terminalRowOffset, terminalColumnOffset);
      
      move_cursor(converter.to_terminal(levelRow, infoCol));
      print("Level:");
      
      move_cursor(converter.to_terminal(bonusRow, infoCol));
      print("Bonus:");
      
      move_cursor(converter.to_terminal(scoreRow, infoCol));
      print("Score:");
      
      move_cursor(converter.to_terminal(nextRow, infoCol));
      print("Next:");
      
      stdio::flush();
  }
  
  fn close<T: Converter>(converter: T, cursorMoveGameRow: i8) {
    reset_graphics();
    show_cursor();
    move_cursor(converter.to_terminal(cursorMoveGameRow, 1));
  }
  
  fn print_score<T: Converter>(converter: T, infoCol: i8, score: Score) {
      reset_graphics();
      
      move_cursor(converter.to_terminal(levelRow, infoCol));
      print!("{}   ", score.level);
      
      move_cursor(converter.to_terminal(bonusRow, infoCol));
      print!("{}    ", score.bonus);
      
      move_cursor(converter.to_terminal(scoreRow, infoCol));
      print!("{}    ", score.score);
  }
  
  pub trait Display {
    fn init(&self);
    fn close(&self);
    fn print_score(&self, score: Score);
    fn print_block(&self, block: Block);
    fn print_next_piece(&self, piece: &Piece);

    fn print_piece(&self, piece: &Piece) {
      for block in piece.blocks.iter() {
        self.print_block(*block);
      }
    }
        
    fn flush(&self) {
      stdio::flush();
    }
    
    fn erase_block(&self, row: i8, col: i8) {
      self.print_block(Block{row: row, column: col, color: Black});
    }
    
    fn erase_piece(&self, piece: &Piece) {
      for block in piece.blocks.iter() {
        self.erase_block(block.row, block.column);
      }
    }
    
    fn erase_next_piece(&self, piece: &Piece) {
      let erase = Piece {
                    ty:     piece.ty,
                    rotate: piece.rotate,
                    blocks: [Block{row:    piece.blocks[0].row,
                                   column: piece.blocks[0].column,
                                   color:  Black},

                             Block{row:    piece.blocks[1].row,
                                   column: piece.blocks[1].column,
                                   color:  Black},

                             Block{row:    piece.blocks[2].row,
                                   column: piece.blocks[2].column,
                                   color:  Black},

                             Block{row:    piece.blocks[3].row,
                                   column: piece.blocks[3].column,
                                   color:  Black}]
      };
      self.print_next_piece(&erase);
    }
  }
  
  pub struct StandardDisplay;

  // terminal level row/column offsets for everything (Blocks, borders, ...)
  static stdRowOffset: i8 = 2i8;
  static stdColumnOffset: i8 = 3i8;
  
  // terminal level number of columns a left/right border takes
  static stdBorderColumns: i8 = 2i8;
  
  impl StandardDisplay {
    #[inline(always)]
    fn to_terminal(row: i8, col: i8) -> (i8, i8) {
      (row + stdRowOffset, 2 * col + stdBorderColumns - 1 + stdColumnOffset)
    }
  }
  
  impl Converter for StandardDisplay {
    fn to_terminal(&self, row: i8, col: i8) -> (i8, i8) {
      StandardDisplay::to_terminal(row, col)
    }
  }
  
  impl Display for StandardDisplay {
    fn init(&self) {
      init(*self, 20, 20, stdRowOffset, stdColumnOffset, baseInfoCol);
    }

    fn close(&self) {
      close(*self, 23);
    }
    
    fn print_score(&self, score: Score) {
      print_score(*self, baseInfoCol + 4, score);
    }
    
    fn print_block(&self, block: Block) {
      if block.row < 1 || block.column < 1 {
        return;
      }
      move_cursor(StandardDisplay::to_terminal(block.row, block.column));
      set_background_color(block.color as u8);
      print("  ");
    }
    
    fn print_next_piece(&self, piece: &Piece) {
      let colOffset = match piece.ty {
        O | S => 13,
        _     => 14
      };
      for block in piece.blocks.iter() {
        move_cursor(StandardDisplay::to_terminal(nextRow + block.row, colOffset + block.column));
        set_background_color(block.color as u8);
        print("  ");
      }
    }
  }
  
  pub struct DoubleDisplay;
  
  static dblRowOffset: i8 = 2i8;
  static dblColumnOffset: i8 = 30i8;
  static dblBorderColumns: i8 = 2i8;
  
  impl DoubleDisplay {
    #[inline(always)]
    fn to_terminal(row: i8, col: i8) -> (i8, i8) {
      (2 * row + dblRowOffset, 4 * col - 3 + dblBorderColumns + dblColumnOffset)
    }
  }
  
  impl Converter for DoubleDisplay {
    fn to_terminal(&self, row: i8, col: i8) -> (i8, i8) {
      DoubleDisplay::to_terminal(row, col)
    }
  }
  
  impl Display for DoubleDisplay {
    fn init(&self) {
      init(*self, 40, 40, dblRowOffset, dblColumnOffset, baseInfoCol - 1);
    }
  
    fn close(&self) {
      close(*self, 22);
    }
  
    fn print_score(&self, score: Score) {
      print_score(*self, baseInfoCol + 1, score);
    }
  
    fn print_block(&self, block: Block) {
       if block.row < 1 || block.column < 1 {
        return;
      }
      move_cursor(DoubleDisplay::to_terminal(block.row, block.column));
      set_background_color(block.color as u8);
      print("    ");
      move_cursor((2 * block.row - 1 + dblRowOffset, 4 * block.column - 3 + dblBorderColumns + dblColumnOffset));
      print("    ");
    }
    
    fn print_next_piece(&self, piece: &Piece) {
      let colOffset = match piece.ty {
        O | S => 10,
        _     => 11
      };
      for block in piece.blocks.iter() {
        move_cursor(DoubleDisplay::to_terminal(nextRow + block.row, colOffset + block.column));
        set_background_color(block.color as u8);
        print("    ");
        move_cursor((2 * (nextRow + block.row) - 1 + dblRowOffset,
                     4 * (colOffset + block.column) - 3 + dblBorderColumns + dblColumnOffset));
        print("    ");
      }
    }
  }
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

  #[inline(always)]
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

mod scoring {
  use std::libc::c_int;

  struct Level {
    time:     c_int,
    score:    int,
    count:    int,
    bonusInc: int
  }
  
  static levels : [Level, ..11] = [Level{time: 1000, score: 0,  count: 3, bonusInc: 1},
                                   Level{time: 900,  score: 5,  count: 3, bonusInc: 1},
                                   Level{time: 800,  score: 10, count: 3, bonusInc: 2},
                                   Level{time: 700,  score: 15, count: 3, bonusInc: 2},
                                   Level{time: 600,  score: 20, count: 3, bonusInc: 3},
                                   Level{time: 500,  score: 30, count: 4, bonusInc: 4},
                                   Level{time: 400,  score: 40, count: 4, bonusInc: 4},
                                   Level{time: 350,  score: 45, count: 5, bonusInc: 5},
                                   Level{time: 300,  score: 50, count: 6, bonusInc: 10},
                                   Level{time: 250,  score: 60, count: 6, bonusInc: 20},
                                   Level{time: 200,  score: 70, count: 4, bonusInc: 20}];
  
  #[inline(always)]
  fn get_level(level: u16) -> &'static Level {
    &levels[level - 1]
  }
  
  #[deriving(Encodable, Decodable)]
  pub struct Score {
    level: u16,
    bonus: int,
    score: int
  }
  
  pub trait Scoring {
    fn get_score(&self) -> Score;
    fn update(&mut self, setRows: int) -> Score;
    fn get_time(&self) -> c_int;
  }
  
  pub fn new() -> ~Scoring {
    ~StdScoring{level:     1,
                score:     0,
                bonus:     1,
                count:     0,
                bonusDrop: bonusDropReset} as ~Scoring
  }
  
  struct StdScoring {
    level:     u16,
    score:     int,
    bonus:     int,
    count:     int,
    bonusDrop: int,
  }
  
  // control how many pieces drop without completing any rows before the bonus is decremented
  static bonusDropReset: int = 1;
  
  impl StdScoring {
    fn update_some_set_rows(&mut self, setRows: int) -> Score {
      let level = get_level(self.level);
      
      let baseScore = 10 * (1 << (setRows - 1));
      let levelScore = level.score;
      
      self.score += (baseScore + levelScore) * self.bonus;
      
      self.count += 1;
      
      // add to the bonus when a level is cleared
      let bonusInc = if self.count > level.count { level.bonusInc } else { 0 };
      
      if self.count > level.count {
        if self.level < levels.len() as u16 {
          self.level += 1;
        }
        self.count = 0;
      }
      
      if self.bonus == 1 {
        self.bonus = 2 * setRows;
      } else {
        self.bonus += 2 * setRows;
      }
      self.bonus += bonusInc;
      
      self.bonusDrop = bonusDropReset;
      
      self.get_score()
    }
    
    fn update_no_set_rows(&mut self) -> Score {
      if self.bonus > 1 {
        self.bonusDrop -= 1;
        if self.bonusDrop == 0 {
          self.bonus -= 1;
          self.bonusDrop = bonusDropReset;
        }
      }
      self.get_score()
    }
  }
  
  impl Scoring for StdScoring {
    fn get_score(&self) -> Score {
      Score{level: self.level, bonus: self.bonus, score: self.score}
    }
    
    fn update(&mut self, setRows: int) -> Score {
      if setRows > 0 {
        self.update_some_set_rows(setRows)
      } else {
        self.update_no_set_rows()
      }
    }
    
    fn get_time(&self) -> c_int {
      get_level(self.level).time
    }
  }
}

mod score_keeper {
  use serialize::json;
  use serialize::{Encodable, Decodable};
  use scoring::Score;
  use std::io::File;
  use time;
  
  pub trait ScoreKeeper {
    fn store_score(&self, tm: &time::Tm, score: Score);
    fn get_scores(&self) -> ScoreStorage;
  }
    
  #[deriving(Encodable, Decodable)]
  pub struct ScoreStorage {
    highScores:   ~[(time::Tm, Score)],
    recentScores: ~[(time::Tm, Score)]
  }
  
  pub fn get() -> &ScoreKeeper {
    &myFileScoreKeeper as &ScoreKeeper
  }

  
  struct FileScoreKeeper;
  
  static myFileScoreKeeper: FileScoreKeeper = FileScoreKeeper;  
  static maxScores : uint = 5;
  
  impl ScoreKeeper for FileScoreKeeper {
    fn store_score(&self, tm: &time::Tm, score: Score) {
      // zero scores aren't worth keeping
      if score.score <= 0 {
        return;
      }
      
      let mut scores = self.get_scores();
      
      scores.highScores.insert(0, (tm.clone(), score));
      scores.highScores.sort_by(|&(_, s1), &(_, s2)| s2.score.cmp(&s1.score));
      if scores.highScores.len() > maxScores {
        scores.highScores.pop();
      }
      
      scores.recentScores.insert(0, (tm.clone(), score));
      if scores.recentScores.len() > maxScores {
        scores.recentScores.pop();
      }
      
      let mut scoresFile = File::create(&Path::new("scores.json"));
      let mut encoder = json::PrettyEncoder::new(&mut scoresFile);
      scores.encode(&mut encoder);
    }
    
    fn get_scores(&self) -> ScoreStorage {
      let emptyStorage = ScoreStorage {
        highScores:   ~[],
        recentScores: ~[]
      };
      
      let storageFile = File::open(&Path::new("scores.json"));
      if storageFile.is_err() {
        return emptyStorage;
      }
      
      let storageObject = json::from_reader(&mut storageFile.unwrap());
      if storageObject.is_err() {
        return emptyStorage;
      }
      
      let mut decoder = json::Decoder::new(storageObject.unwrap());
      Decodable::decode(&mut decoder)
    }
  }
}

mod tetris {
  use time;
  use std::libc::c_int;
  
  use terminal_control;
  use input_reader;
  use pieces;
  use pieces::{Block, Piece};
  use graphics::Display;
  use piece_getter;
  use piece_getter::PieceGetter;
  use scoring;
  use scoring::Scoring;
  use score_keeper;
  use score_keeper::ScoreKeeper;
  use set_blocks::SetBlocks;
  
  trait GameHandler {
    fn init(&self);
    fn handle_step(&mut self) -> Option<c_int>;
    fn handle_input(&mut self, input: input_reader::ReadResult);
    fn handle_quit(&self);
  }

  enum State {
    Fall = 0, Clear, GameOver
  }

  struct TetrisGame<'a> {
    display:     &'a Display,
    pieceGetter: &'a mut PieceGetter,
    scoring:     &'a mut Scoring,
    scoreKeeper: &'a ScoreKeeper,
    state:       State,
    piece:       Piece,
    nextPiece:   Piece,
    setBlocks:   [Option<Block>, ..200]
  }

  impl<'a> TetrisGame<'a> {  
    fn collides_with_set_blocks(&self, piece: &Piece) -> bool {
      piece.blocks.iter().any(|block| self.setBlocks.has_block(block.row, block.column))
    }
    
    fn in_bounds_bottom_row(piece: &Piece) -> bool {
      piece.blocks.iter().all(|block| block.row <= 20)
    }
    
    fn in_bounds_cols(piece: &Piece) -> bool {
      piece.blocks.iter().all(|block| block.column >= 1 && block.column <= 10)
    }
    
    fn all_in_bounds(piece: &Piece) -> bool {
      piece.blocks.iter().all(|block| block.row >= 1 && block.row <= 20 && block.column >= 1 && block.column <= 10)
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
    
    fn erase_row(&self, row: i8) {
      for col in range(1, 11i8) {
        self.display.erase_block(row, col);
      }
    }
    
    fn erase_set_rows(&self) {
      for row in range(1, 21i8) {
        if self.is_row_set(row) {
          self.erase_row(row);
        }
      }
    }
    
    fn erase_all_set_blocks(&self) {
      for row in range(1, 21i8) {
        for col in range(1, 11i8) {
          match self.setBlocks.get(row, col) {
            None    => (),
            Some(_) => self.display.erase_block(row, col)
          }
        }
      }
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
      self.display.erase_piece(&self.piece);
      
      self.piece = *next;
      
      self.display.print_piece(&self.piece);
    }
    
    fn go_to_next_piece(&mut self) {
        self.set_piece();
        
        self.display.erase_next_piece(&self.nextPiece);
        
        self.piece = self.nextPiece;
        self.nextPiece = self.pieceGetter.next_piece();
        
        self.display.print_next_piece(&self.nextPiece);
    }
    
    fn step_fall(&mut self) -> Option<c_int> {
      match self.can_move_rows(&self.piece, 1) {
        true  => {
          let translated = pieces::translate(&self.piece, 1, 0);
          self.update_piece(&translated);
          
          Some(self.scoring.get_time())
        }
        false => {
          if !TetrisGame::all_in_bounds(&self.piece) {
            self.state = GameOver;
            return Some(500);
          }
          
          self.go_to_next_piece();
          
          let setRows = self.set_row_count();
          if setRows > 0 {
            self.erase_set_rows();
            self.state = Clear;
          }
          
          let s = self.scoring.update(setRows);
          self.display.print_score(s);
          
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
      self.scoreKeeper.store_score(&time::now(), self.scoring.get_score());
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
    fn init(&self) {
      self.display.print_next_piece(&self.nextPiece);
      self.display.print_score(self.scoring.get_score());
      self.display.flush();
    }
    
    fn handle_step(&mut self) -> Option<c_int> {    
      let stepTime = 
      match self.state {
        Fall     => self.step_fall(),
        Clear    => self.step_clear(),
        GameOver => self.step_game_over()
      };
      self.display.flush();
      stepTime
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
      self.display.flush();
    }
    
    fn handle_quit(&self) {
      self.scoreKeeper.store_score(&time::now(), self.scoring.get_score());
    }
  }

  fn main_loop<T: GameHandler>(handler: &mut T) {
    use input_reader::{poll_stdin, read_stdin, Other, PollReady, PollTimeout};
    
    handler.init();
    
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
            Other => {
              handler.handle_quit();
              break;
            }
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

  pub fn run_game(display: &Display) {
    // the restorer resets the terminal out of raw mode once it's dropped
    let _restorer = terminal_control::set_terminal_raw_mode();
    
    display.init();
    
    let mut scoring = scoring::new();
    
    let scoreKeeper = score_keeper::get();
    
    let mut pieceGetter = piece_getter::new();
    let firstPiece = pieceGetter.next_piece();
    let secondPiece = pieceGetter.next_piece();

    display.print_next_piece(&secondPiece);
    
    let mut game = TetrisGame{display:     display,
                              pieceGetter: pieceGetter,
                              scoring:     scoring,
                              scoreKeeper: scoreKeeper,
                              state:       Fall,
                              piece:       firstPiece,
                              nextPiece:   secondPiece,
                              setBlocks:   [None, ..200]};

    main_loop(&mut game);
    
    display.close();
  }
}

fn display_help() {
  println("");
  println("A simple game of Tetris implemented in Rust");
  println("");
  println("Options:");
  println("--help or -h             |  show this help");
  println("--scores                 |  show scores");
  println("--display=double or -d2  |  run in double display mode");
  println("");
  println("Controls:");
  println("left arrow     | move piece left");
  println("right arrow    | move piece right");
  println("up arrow       | rotate piece");
  println("down arrow     | quick drop piece");
  println("any other key  | exit the game");
  println("");
  println("Run this program with no arguments to start a game in standard display mode");
  println("");
}

fn display_scores() {
/*
High Scores:                   Recent Scores:
Thu Jan  1 00:00:00 1970       Thu Jan 1 00:00:00 1970
level: 1                       level: 1
bonus: 1                       bonus: 1
score: 1                       score: 1

Thu Jan  1 00:00:00 1970       Thu Jan 1 00:00:00 1970
level: 1                       level: 1
bonus: 1                       bonus: 1
score: 1                       score: 1

Thu Jan  1 00:00:00 1970       Thu Jan 1 00:00:00 1970
level: 1                       level: 1
bonus: 1                       bonus: 1
score: 1                       score: 1

Thu Jan  1 00:00:00 1970       Thu Jan 1 00:00:00 1970
level: 1                       level: 1
bonus: 1                       bonus: 1
score: 1                       score: 1

Thu Jan  1 00:00:00 1970       Thu Jan 1 00:00:00 1970
level: 1                       level: 1
bonus: 1                       level: 1
score: 1                       level: 1
*/

  fn digits(i: int) -> int {
    match i {
      0 .. 9             => 1,
      10 .. 99           => 2,
      100 .. 999         => 3,
      1000 .. 9999       => 4,
      10000 .. 99999     => 5,
      100000 .. 999999   => 6,
      1000000 .. 1000000 => 7,
      _                  => 8
    }
  }
  
  fn print_spaces(n: int) {
    for _ in range(0, n) {
      print(" ");
    }
  }
  
  fn max(a: uint, b: uint) -> uint {
    if a >= b {
      a
    } else {
      b
    }
  }
  
  println("");
  println("High Scores:                   Recent Scores:");
  
  let scores = &score_keeper::get().get_scores();  
  
  let n = max(scores.highScores.len(), scores.recentScores.len());

  for i in range(0, n) {
    if i < scores.highScores.len() && i < scores.recentScores.len() {
      let (ref highScoreTm, ref highScoreScore) = scores.highScores[i];
      let (ref recentScoreTm, ref recentScoreScore) = scores.recentScores[i];
      println!("{}       {}", highScoreTm.ctime(), recentScoreTm.ctime());
        
      print!("level: {}", highScoreScore.level);
      print_spaces(24 - digits(highScoreScore.level as int));
      println!("level: {}", recentScoreScore.level);
      
      print!("bonus: {}", highScoreScore.bonus);
      print_spaces(24 - digits(highScoreScore.bonus));
      println!("bonus: {}", recentScoreScore.bonus);
      
      print!("score: {}", highScoreScore.score);
      print_spaces(24 - digits(highScoreScore.score));
      println!("score: {}", recentScoreScore.score);
    
    } else if i < scores.highScores.len() {
      let (ref highScoreTm, ref highScoreScore) = scores.highScores[i];
      println!("{}", highScoreTm.ctime());
      println!("level: {}", highScoreScore.level);
      println!("bonus: {}", highScoreScore.bonus);
      println!("score: {}", highScoreScore.score);
    
    } else if i < scores.recentScores.len() {
      let (ref recentScoreTm, ref recentScoreScore) = scores.recentScores[i];
      println!("                               {}", recentScoreTm.ctime());
      println!("                               level: {}", recentScoreScore.level);
      println!("                               bonus: {}", recentScoreScore.bonus);
      println!("                               score: {}", recentScoreScore.score);
    }
    println("");
  }
}

fn main() {
  let args = os::args();
  
  // There's always at least one argument (the program's name)
  // If the program is run with no extra argument's passed by the user, just run the game in standard display mode
  //
  // Otherwise there are at least two arguments, handle double display or help argument.
  // If we don't understand the argument, just show the help
  match args.len() {
    1 => tetris::run_game(&graphics::StandardDisplay),
    _ => {
      match args[1] {
        ~"--help" | ~"-h"            => display_help(),
        ~"--score" | ~"--scores"     => display_scores(),
        ~"--display=double" | ~"-d2" => tetris::run_game(&graphics::DoubleDisplay),
        _                            => display_help()
      }
    }
  }
}
