extern mod extra;
use extra::time;

use std::libc::c_int;

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// terminal control

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

/*
enum Color {
  Black = 0, Red, Green, Yellow, Blue, Magenta, Cyan, White
}

struct Block {
  row: u8,
  column: u8,
  color: Color
}

struct LinkedBlock {
  block: Block,
  next: Option<~LinkedBlock>
}

fn csi() {
  print!("{}[", '\x1B');
}

fn reset() {
  csi();
  print("m");
}

fn clear_display() {
  csi();
  print("2J");
}

fn move_cursor(row: u8, column: u8) {
  csi();
  print!("{};{}H", row, column);
}

fn print_block(block: Block) {  
  move_cursor(block.row, block.column);
  csi();
  print!("{}m ", 40 + (block.color as u8));
}

fn init() {
  
}
*/

fn main_loop(handle_step: || -> (), handle_direction: |input_reader::ReadResult| -> ()) {
  use input_reader::{poll_stdin, read_stdin, Other, PollReady, PollTimeout};
  
  // milliseconds between piece drop steps
  let stepTimeMs: c_int = 3000;
  
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
	    println!("{}", pollTimeMs);
	    handle_direction(input);
	  }
	}
      }
      PollTimeout => {
	pollTimeMs = stepTimeMs;
	sinceLastStepNs = 0;
	handle_step();
      }
    }
  }  
}

fn handle_step() {
  println(" s ");
}

fn handle_direction(input: input_reader::ReadResult) {
  use input_reader::{Up, Down, Right, Left};
  match input {
    Up    => println(" Up "),
    Down  => println(" Down "),
    Right => println(" Right "),
    Left  => println(" Left "),
    _     => fail!("unknown direction")
  }
}

fn main() {

  
  /*
  //println!("size_of Block = {}", size_of::<Block>());
  //println!("size_of LinkedBlock = {}", size_of::<LinkedBlock>());
  //println!("size_of c_int = {}", size_of::<c_int>());
  //println!("size_of termios = {}", size_of::<termios>());

  //println!("size_of c_long = {}", size_of::<c_long>());
  //println!("size_of pollfd = {}", size_of::<pollfd>());
  
//   clear_display();
//   move_cursor(5,5);
//   print("this is a test");
//   
//   print_block(Block{row: 6, column: 5, color: Red});
//   print_block(Block{row: 6, column: 6, color: Red});
//   print_block(Block{row: 6, column: 7, color: Red});
//   print_block(Block{row: 6, column: 8, color: Red});
//   
//   reset();
  */
  
  let restorer = terminal_control::set_terminal_raw_mode();
  
  main_loop(handle_step, handle_direction);
  
  restorer.restore();
}
