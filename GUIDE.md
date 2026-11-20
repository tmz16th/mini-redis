# mini-redis, explained
r
| Situation | What to do |
|---|---|
| Normal way to stop your server | Press **Ctrl+C** in the terminal where it's running |
| Forgot where it's running | `ss -ltnp \| grep 6379`, then `kill <the number>` |
| Can't be bothered | Change the port in `main.rs` from `6379` to `6380` |

`ss -ltnp` asks the computer "who is holding this phone number?".

---

## 1. What am i actually building?

A **key-value store** is a fancy name for **a labelled toy box**.

You put a toy in with a label:

```
"name" → "Ted"
```

Later you say "give me the thing labelled `name`" and it gives you `Ted`.

A `HashMap` in Rust is exactly this toy box, but it lives inside one program.
This project takes that toy box and **puts a phone line on it**, so other
programs can call up and say "put this in", "give me this", "throw this away".

That's the whole idea. Everything else is about making it **safe**, **fast**,
and **not forgetting things**.

---

## 2. `store.rs` — the toy box and the notebook

### The struct

```rust
pub struct Store {
    data: HashMap<String, String>,
    log: Option<BufWriter<File>>,
}
```

A **struct** is a cardboard box with labelled compartments. Our box has two:

- `data` — the toy box (the `HashMap`)
- `log` — a notebook we write in

Why `Option<...>` for the notebook? `Option` means **"maybe there is one, maybe
there isn't"**:

- `Some(notebook)` → a real store, writes things down
- `None` → a pretend store used in tests, no notebook needed

Tests don't need a real file, so we give them a store with no notebook. That's
what `in_memory()` does.

### `impl` — the instruction manual

```rust
impl Store { ... }
```

`impl` means "here are the things this box can DO". Without it, `Store` is just
a dumb container. With it, it can `set`, `get`, `remove`, `open`, and `replay`.

### `&self` versus `&mut self` — looking versus touching

This is the big Rust idea:

```rust
pub fn get(&self, ...)      // &self = "I'm only LOOKING"
pub fn set(&mut self, ...)  // &mut self = "I'm CHANGING something"
```

Rust's rule: **any number of people can look, but only one can change, and
nobody looks while someone is changing.**

Why? Imagine two people both changing the toy box at once — chaos. Rust refuses
to compile code that could do that. It's annoying sometimes, but it's exactly
why Rust programs don't crash in weird ways.

### `Option` — the maybe-box

```rust
pub fn get(&self, key: &str) -> Option<&str>
```

When you ask for a key, two things can happen:

- `Some("Ted")` — found it!
- `None` — nothing there

Rust **forces** you to handle both. It won't let you accidentally assume the
key exists. That's why in the server we write:

```rust
match s.get(key) {
    Some(value) => format!("{value}\n"),
    None => "(nil)\n".to_string(),
}
```

`match` is like a fork in the road: "if it's this, do that; if it's that, do
this."

### `Result` — the "did it work?" envelope

```rust
pub fn set(&mut self, key: String, value: String) -> io::Result<()>
```

Writing to a disk can fail — disk full, file deleted, permissions. So `set`
now returns:

- `Ok(())` — "I did it"
- `Err(e)` — "I failed, here's why: `e`"

We use `?` to pass the problem upward:

```rust
self.append(format!("SET {key} {value}\n"))?;
```

The `?` means: *"if this failed, stop here and hand the error to whoever called
me. If it worked, carry on."*

And `unwrap()` means: *"I'm SURE this worked. If I'm wrong, crash loudly."* We
only use that in tests, where crashing is fine.

### The notebook — WAL

**WAL** = Write-Ahead Log. The rule is simple:

> **Write it in the notebook BEFORE you change the toy box.**

Why? Because if the power cuts right after you write in the notebook, you can
read the notebook later and rebuild the toy box. If you changed the toy box
*first* and died before writing, the change is lost forever.

That's why `set` looks like this:

```rust
self.append(format!("SET {key} {value}\n"))?;  // 1. notebook
self.apply_set(key, value);                    // 2. toy box
```

Order matters. Notebook first.

### `apply_set` versus `set` — why two versions?

When we start the program, we read the notebook to rebuild the toy box. That's
`replay()`. But if `replay()` called `set()`, then `set()` would write into the
notebook *again*. Every restart would duplicate the whole notebook. Eventually
it would fill the disk.

So we have two versions:

| Function | Notebook? | Toy box? | Used by |
|---|---|---|---|
| `set` | yes, writes | yes, changes | real clients |
| `apply_set` | no | yes, changes | `replay()` only |

`apply_set` is the quiet version. Same change, no writing.

### `fsync` — "I really mean it"

```rust
log.write_all(record.as_bytes())?;
log.flush()?;
log.get_ref().sync_data()?;
```

Three steps, like posting a letter:

1. `write_all` — put the words on the paper
2. `flush` — hand the letter to the postman (the operating system)
3. `sync_data` — make the postman actually deliver it, not just hold it in his bag

If you skip step 3, the computer might hold your letter in memory and lose it
in a power cut. That's what "durability" means.

It's **slow** — roughly 197 writes/sec instead of 785,000. Real databases cheat
by only doing it once a second. We do it every time so it's simple and correct.

### Torn writes

If the power cuts *while* writing, the notebook might end like this:

```
SET good value
SET bad hal
```

The last line is half a word. When we read it back, we notice it doesn't end
with a newline `\n`, so we **chop it off** with `set_len()` and keep everything
before it.

We read **bytes**, not text, because half a letter could be half of a funny
character like `é`. Reading bytes means we don't crash on weird half-characters.

---

## 3. `main.rs` — the phone line

### `main()` — the front door

Every Rust program starts in `main()`. Ours:

1. Opens the notebook and replays it (`Store::open`)
2. Rings the doorbell (`TcpListener::bind`)
3. Waits for callers in a loop

### `TcpListener` — the doorbell

```rust
let listener = TcpListener::bind(ADDR)?;
for stream in listener.incoming() { ... }
```

`bind` claims the phone number. `incoming()` is an endless loop of "who's
calling? who's calling? who's calling?" Each caller gives us a `TcpStream` — a
phone line to that one person.

### Threads — one helper per caller

```rust
thread::spawn(move || { handle_client(stream, store) });
```

If one caller is slow (typing very slowly), we don't want everyone else
waiting. So we hire a **new helper for each caller**. Each helper gets their own
phone line and gets on with it.

### `Arc` and `RwLock` — the shared cupboard

Here's the problem: 20 helpers, but only **one toy box**. If each helper got
their own copy, they'd each see different data. Useless.

Two tools solve it:

**`Arc`** — give every helper the **same key** to the cupboard. Not copies of
the cupboard — the same one.

**`RwLock`** — the rule on the cupboard door:

- Lots of helpers can **peek** at once (`read()`)
- Only **one** helper can change things (`write()`)
- Nobody peeks while someone's changing

```rust
let s = store.read().unwrap();        // GET — peeking
let mut s = store.write().unwrap();   // SET/DEL — changing
```

The `write()` helper holds the key the whole time we write to the notebook.
That's correct — writes must be one at a time, in order.

`Arc::clone(&store)` doesn't copy the store. It copies the **key**. That's the
whole point.

### `BufReader` — reading one line at a time

```rust
let mut reader = BufReader::new(stream.try_clone()?);
reader.read_line(&mut line)?
```

A caller sends `SET name Ted\n`. We don't want to deal with one letter at a
time (`S`, `e`, `t`, ...). `read_line` waits until it sees the `\n` (Enter) and
hands us the whole sentence.

`try_clone()` makes a second phone line to the same caller, so we can **read**
with one and **write** with the other at the same time.

### `split_whitespace` and `[_, key, value]` — chopping the sentence

```rust
let args: Vec<&str> = line.trim().split_whitespace().collect();
```

`SET name Ted` becomes a list: `["SET", "name", "Ted"]`.

Then:

```rust
match args.as_slice() {
    [_, key, value] => { ... }  // exactly 3 words?
    _ => "ERR usage: SET <key> <value>"  // otherwise, complain
}
```

That pattern checks the *shape*. `_` means "I don't care about this one" (the
command word, already handled above). If someone types just `SET`, the shape is
wrong and we print the usage message instead of crashing.

---

## 4. The commands you'll actually type

### Cargo (building and running)

| Command | What it really does |
|---|---|
| `cargo new name` | Makes a new project folder with all the boring bits set up |
| `cargo build` | Turns your `.rs` files into a program (slow, debug version) |
| `cargo run` | `cargo build` then immediately run it |
| `cargo test` | Runs all the `#[test]` functions |
| `cargo build --release` | Builds the **fast** version (slower to build, much faster to run) |
| `cargo test --release -- --ignored --nocapture` | Runs the slow benchmarks and **prints** the numbers (`--nocapture` = "don't hide my `println!`") |

Always benchmark with `--release`. Debug builds are like running with the
handbrake on.

### Git (save points)

| Command | What it really does |
|---|---|
| `git status` | "Where am I, what have I changed?" |
| `git add <file>` | Put a change into the **ready-to-save** pile |
| `git commit -m "..."` | Take a **photo** of the ready pile, with a label |
| `git checkout -b name` | Make a new **timeline** starting from here (a branch) |
| `git branch` | List the timelines |
| `git push` | Upload your photos to GitHub |
| `git log --oneline` | Show the photos, newest first |

We did this so stage 3 lives on its own timeline. If it goes wrong, `master` is
still safe.

### Talking to the server

```bash
nc localhost 6379
```

`nc` (netcat) is just a dumb terminal that types whatever you type into the
phone line. Type `PING`, press Enter, get `PONG`.

---

## 5. Quick "when do I use what?" cheat sheet

| I want to... | Use |
|---|---|
| Look at something without changing it | `&self`, `store.read()` |
| Change something | `&mut self`, `store.write()` |
| Something might not exist | `Option` + `match` |
| Something might fail | `Result` + `?` (or `unwrap()` in tests) |
| Read text line by line | `BufReader::read_line` |
| Write text to a file | `BufWriter::write_all` |
| Make text from numbers | `format!` or `.to_string()` |
| Check a list has the right shape | slice pattern `[_, key, value]` |
| Save my work | `git add` then `git commit` |
| Try something risky | `git checkout -b <name>` |

---
