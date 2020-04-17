extern crate copperline;
extern crate golfscript;

fn main() {
    let mut it = golfscript::Interpreter::new();
    let mut rl = copperline::Copperline::new();

    while let Ok(line) = rl.read_line_utf8(">> ") {
        match it.exec(&line) {
            Ok(response) => {
                for el in response {
                    print!("| {} ", el);
                }
                println!("|");
                rl.add_history(line);
            }

            Err(err) => {
                println!("{:?}", err);
            }
        }
    }
}
