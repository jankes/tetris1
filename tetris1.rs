

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

// 
mod input_reader {
  use std::libc::{c_int, c_short, c_long};
  
  pub enum PollResult {
    PollReady,
    PollTimeout,
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
  
  pub fn read_stdin() -> u8 {
    unsafe {
      let mut c = 0u8;
      
      // first parameter is file descriptor number, 0 ==> standard input
      let numRead = read(0, &mut c, 1);
      if numRead < 0 {
	fail!("error reading standard input");
      }
      return c;
    }
  }
}

//fn 


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

fn main() {
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

  //disable_canonical_input();
  /*  
  match poll_stdin(5000) {
    PollReady   => println("input ready"),
    PollTimeout => println("timeout"),
    PollError   => println("error")
  }
  */

  /*
  use input_reader::{poll_stdin, PollReady, PollTimeout, read_stdin};
  
  let restorer = terminal_control::set_terminal_raw_mode();
  
  match poll_stdin(5000) {
    PollReady => {
      let c = read_stdin();
      println!("read {}", c);
    }
    PollTimeout => println("timeout!")
  }
  
  restorer.restore();
  */
  
  
}
