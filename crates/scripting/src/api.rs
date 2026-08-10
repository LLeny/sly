use std::rc::Rc;
use std::string::String as StdString;
use std::sync::Arc;

use core_model::Button;
use debugger_model::{BreakpointKind, ConsoleLevel};
use piccolo::{Callback, CallbackReturn, Context, Error, FromValue, Function, Stack, Table, Value};

use crate::host::{Host, Trigger};

type Builtin =
    for<'gc> fn(&Host, Context<'gc>, Stack<'gc, '_>) -> Result<CallbackReturn<'gc>, Error<'gc>>;

pub fn install(ctx: Context<'_>, host: &Rc<Host>) {
    let sly = Table::new(&ctx);

    for (name, call) in [
        ("read8", read8 as Builtin),
        ("read16", read16),
        ("read24", read24),
        ("read", read),
        ("write8", write8),
        ("write16", write16),
        ("write", write),
        ("regions", regions),
        ("find", find),
        ("cpus", cpus),
        ("pc", pc),
        ("setpc", setpc),
        ("reg", reg),
        ("setreg", setreg),
        ("registers", registers),
        ("flags", flags),
        ("on_exec", on_exec),
        ("on_read", on_read),
        ("on_write", on_write),
        ("on_frame", on_frame),
        ("on_event", on_event),
        ("events", events),
        ("remove", remove),
        ("clear_hooks", clear_hooks),
        ("pause", pause),
        ("resume", resume),
        ("button", button),
        ("frame", frame),
        ("status", status),
        ("log", log),
        ("warn", warn),
        ("error", error_line),
        ("hex", hex),
        ("dump", dump),
    ] {
        bind(ctx, sly, host, name, call);
    }

    ctx.set_global("sly", sly).ok();

    let printer = host.clone();
    let print = Callback::from_fn(&ctx, move |ctx, _, stack| {
        printer.print(ConsoleLevel::Print, joined(ctx, stack));
        Ok(CallbackReturn::Return)
    });
    ctx.set_global("print", print).ok();

    self::install_stdlib_gaps(ctx);
}

fn install_stdlib_gaps(ctx: Context<'_>) {
    let tonumber = Callback::from_fn(&ctx, |ctx, _, mut stack| {
        let value: Value = stack.consume(ctx)?;
        let parsed = match value {
            Value::Integer(int) => Some(int),
            Value::Number(number) => Some(number as i64),
            Value::String(text) => {
                let text = std::str::from_utf8(text.as_bytes())
                    .unwrap_or_default()
                    .trim();
                match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    Some(digits) => i64::from_str_radix(digits, 16).ok(),
                    None => text.parse().ok(),
                }
            }
            _ => None,
        };

        match parsed {
            Some(number) => stack.replace(ctx, number),
            None => stack.replace(ctx, Value::Nil),
        }
        Ok(CallbackReturn::Return)
    });
    ctx.set_global("tonumber", tonumber).ok();

    let format = Callback::from_fn(&ctx, |ctx, _, mut stack| {
        let values: Vec<Value> = stack.drain(..).collect();
        let Some(Value::String(spec)) = values.first().copied() else {
            return Err(self::fail(ctx, "string.format needs a format string"));
        };

        let text = self::format_string(
            std::str::from_utf8(spec.as_bytes()).unwrap_or_default(),
            &values[1..],
        );
        stack.replace(ctx, ctx.intern(text.as_bytes()));
        Ok(CallbackReturn::Return)
    });

    if let Value::Table(string) = ctx.get_global("string") {
        string.set(ctx, "format", format).ok();
    }
}

fn format_string(spec: &str, args: &[Value<'_>]) -> StdString {
    let mut out = StdString::new();
    let mut next = 0;
    let mut chars = spec.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }

        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }

        let mut flags = StdString::new();
        while chars
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | ' ' | '#' | '.'))
        {
            flags.push(chars.next().unwrap_or_default());
        }

        let Some(kind) = chars.next() else { break };
        let value = args.get(next).copied().unwrap_or(Value::Nil);
        next += 1;

        let (pad_zero, width, precision) = self::format_flags(&flags);
        let body = match kind {
            'd' | 'i' | 'u' => self::as_integer(value).to_string(),
            'x' => format!("{:x}", self::as_integer(value)),
            'X' => format!("{:X}", self::as_integer(value)),
            'o' => format!("{:o}", self::as_integer(value)),
            'c' => char::from_u32(self::as_integer(value) as u32)
                .map(StdString::from)
                .unwrap_or_default(),
            'f' | 'F' => format!("{:.*}", precision.unwrap_or(6), self::as_number(value)),
            'e' => format!("{:e}", self::as_number(value)),
            'g' => format!("{}", self::as_number(value)),
            's' | 'q' => self::value_text(value),
            other => {
                out.push('%');
                out.push(other);
                next -= 1;
                continue;
            }
        };

        let pad = width.saturating_sub(body.chars().count());
        if flags.starts_with('-') {
            out.push_str(&body);
            out.extend(std::iter::repeat_n(' ', pad));
        } else {
            let fill = if pad_zero { '0' } else { ' ' };
            match body.strip_prefix('-').filter(|_| pad_zero) {
                Some(rest) => {
                    out.push('-');
                    out.extend(std::iter::repeat_n(fill, pad));
                    out.push_str(rest);
                }
                None => {
                    out.extend(std::iter::repeat_n(fill, pad));
                    out.push_str(&body);
                }
            }
        }
    }

    out
}

fn format_flags(flags: &str) -> (bool, usize, Option<usize>) {
    let (width, precision) = flags
        .split_once('.')
        .map(|(width, precision)| (width, precision.parse().ok()))
        .unwrap_or((flags, None));

    let digits: StdString = width.chars().filter(char::is_ascii_digit).collect();
    let pad_zero = digits.starts_with('0');

    (pad_zero, digits.parse().unwrap_or(0), precision)
}

fn as_integer(value: Value<'_>) -> i64 {
    match value {
        Value::Integer(int) => int,
        Value::Number(number) => number as i64,
        Value::Boolean(set) => set as i64,
        _ => 0,
    }
}

fn as_number(value: Value<'_>) -> f64 {
    match value {
        Value::Integer(int) => int as f64,
        Value::Number(number) => number,
        _ => 0.0,
    }
}

fn bind<'gc>(
    ctx: Context<'gc>,
    table: Table<'gc>,
    host: &Rc<Host>,
    name: &'static str,
    call: Builtin,
) {
    let host = host.clone();
    let callback = Callback::from_fn(&ctx, move |ctx, _, stack| call(&host, ctx, stack));
    table.set(ctx, name, callback).ok();
}

fn fail<'gc>(ctx: Context<'gc>, message: impl AsRef<str>) -> Error<'gc> {
    Error::from_value(Value::String(ctx.intern(message.as_ref().as_bytes())))
}

fn detached<'gc>(ctx: Context<'gc>) -> Error<'gc> {
    self::fail(ctx, "no emulator core is attached")
}

fn joined<'gc>(ctx: Context<'gc>, mut stack: Stack<'gc, '_>) -> StdString {
    let values: Vec<Value<'gc>> = stack.drain(..).collect();
    values
        .iter()
        .map(|value| match value {
            Value::String(_) => self::value_text(*value),
            _ => self::render(ctx, *value, DUMP_DEPTH),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn value_text(value: Value<'_>) -> StdString {
    let mut text = Vec::new();
    value.display(&mut text).ok();
    StdString::from_utf8_lossy(&text).into_owned()
}

const DUMP_DEPTH: usize = 4;
const DUMP_ENTRIES: usize = 64;

fn render<'gc>(ctx: Context<'gc>, value: Value<'gc>, depth: usize) -> StdString {
    let Value::Table(table) = value else {
        return match value {
            Value::String(_) => format!("\"{}\"", self::value_text(value)),
            _ => self::value_text(value),
        };
    };

    if depth == 0 {
        return "{...}".to_string();
    }

    let length = table.length();
    let array = length > 0
        && (1..=length).all(|ix| !matches!(table.get(ctx, ix), Value::Nil))
        && table.iter().count() as i64 == length;

    let mut parts = Vec::new();
    let mut truncated = 0;

    if array {
        for ix in 1..=length {
            if parts.len() >= DUMP_ENTRIES {
                truncated = length - parts.len() as i64;
                break;
            }
            parts.push(self::render(ctx, table.get(ctx, ix), depth - 1));
        }
    } else {
        let mut entries: Vec<(StdString, Value<'gc>)> = table
            .iter()
            .map(|(key, value)| (self::value_text(key), value))
            .collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        truncated = entries.len().saturating_sub(DUMP_ENTRIES) as i64;
        entries.truncate(DUMP_ENTRIES);
        parts = entries
            .into_iter()
            .map(|(key, value)| format!("{key} = {}", self::render(ctx, value, depth - 1)))
            .collect();
    }

    if truncated > 0 {
        parts.push(format!("... {truncated} more"));
    }

    if parts.is_empty() {
        return "{}".to_string();
    }

    format!("{{ {} }}", parts.join(", "))
}

fn word(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .enumerate()
        .map(|(ix, byte)| (*byte as u32) << (8 * ix))
        .sum()
}

fn resolve_region<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    region: Option<&str>,
) -> Result<Arc<str>, Error<'gc>> {
    let state = host.state();
    state.region(None, region).ok_or_else(|| {
        let known: Vec<&str> = state.region_names().map(|id| &**id).collect();
        self::fail(
            ctx,
            format!(
                "unknown region `{}`, this core has {known:?}",
                region.unwrap_or_default()
            ),
        )
    })
}

fn read_region<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    addr: i64,
    len: usize,
    region: Option<StdString>,
) -> Result<Vec<u8>, Error<'gc>> {
    let region = self::resolve_region(host, ctx, region.as_deref())?;
    host.with_core(|core| core.read_memory(&region, addr as u32, len))
        .ok_or_else(|| self::detached(ctx))
}

fn write_region<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    addr: i64,
    bytes: &[u8],
    region: Option<StdString>,
) -> Result<usize, Error<'gc>> {
    let region = self::resolve_region(host, ctx, region.as_deref())?;
    host.with_core(|core| core.write_memory(&region, addr as u32, bytes))
        .ok_or_else(|| self::detached(ctx))
}

fn read_width<'gc>(
    len: usize,
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (addr, region): (i64, Option<StdString>) = stack.consume(ctx)?;
    let bytes = self::read_region(host, ctx, addr, len, region)?;
    stack.replace(ctx, self::word(&bytes) as i64);
    Ok(CallbackReturn::Return)
}

fn read8<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::read_width(1, host, ctx, stack)
}

fn read16<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::read_width(2, host, ctx, stack)
}

fn read24<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::read_width(3, host, ctx, stack)
}

fn read<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    const MAX_READ: i64 = 0x1000;

    let (addr, len, region): (i64, i64, Option<StdString>) = stack.consume(ctx)?;
    let bytes = self::read_region(host, ctx, addr, len.clamp(0, MAX_READ) as usize, region)?;

    let table = Table::new(&ctx);
    for (ix, byte) in bytes.iter().enumerate() {
        table.set(ctx, ix as i64 + 1, *byte as i64).ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn write_width<'gc>(
    len: usize,
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (addr, value, region): (i64, i64, Option<StdString>) = stack.consume(ctx)?;
    let bytes: Vec<u8> = (0..len).map(|ix| (value >> (8 * ix)) as u8).collect();
    let written = self::write_region(host, ctx, addr, &bytes, region)?;

    stack.replace(ctx, written == len);
    Ok(CallbackReturn::Return)
}

fn write8<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::write_width(1, host, ctx, stack)
}

fn write16<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::write_width(2, host, ctx, stack)
}

fn write<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (addr, values, region): (i64, Table<'gc>, Option<StdString>) = stack.consume(ctx)?;

    let mut bytes = Vec::with_capacity(values.length().max(0) as usize);
    for ix in 1..=values.length() {
        bytes.push(self::as_integer(values.get(ctx, ix)) as u8);
    }

    let written = self::write_region(host, ctx, addr, &bytes, region)?;
    stack.replace(ctx, written as i64);
    Ok(CallbackReturn::Return)
}

fn regions<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let table = Table::new(&ctx);
    for (ix, region) in host.state().region_names().enumerate() {
        table
            .set(ctx, ix as i64 + 1, ctx.intern(region.as_bytes()))
            .ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn cpus<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let table = Table::new(&ctx);
    for (ix, cpu) in host.state().cpus.iter().enumerate() {
        table
            .set(ctx, ix as i64 + 1, ctx.intern(cpu.id.as_str().as_bytes()))
            .ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn cpu_id<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    name: Option<&str>,
) -> Result<debugger_model::CpuId, Error<'gc>> {
    host.state()
        .cpu(name)
        .map(|cpu| cpu.id.clone())
        .ok_or_else(|| self::fail(ctx, format!("unknown cpu `{}`", name.unwrap_or_default())))
}

fn pc<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let cpu: Option<StdString> = stack.consume(ctx)?;
    let id = self::cpu_id(host, ctx, cpu.as_deref())?;
    let pc = host
        .with_core(|core| core.pc(&id))
        .ok_or_else(|| self::detached(ctx))?;

    stack.replace(ctx, pc as i64);
    Ok(CallbackReturn::Return)
}

fn setpc<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (addr, cpu): (i64, Option<StdString>) = stack.consume(ctx)?;
    let id = self::cpu_id(host, ctx, cpu.as_deref())?;
    host.with_core(|core| core.set_pc(&id, addr as u32))
        .ok_or_else(|| self::detached(ctx))?;

    Ok(CallbackReturn::Return)
}

fn reg<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (name, cpu): (StdString, Option<StdString>) = stack.consume(ctx)?;
    let id = self::cpu_id(host, ctx, cpu.as_deref())?;
    let value = host
        .with_core(|core| {
            core.cpu_view(&id)
                .registers
                .iter()
                .find(|reg| reg.name.eq_ignore_ascii_case(&name))
                .map(|reg| reg.value)
        })
        .ok_or_else(|| self::detached(ctx))?;

    match value {
        Some(value) => stack.replace(ctx, value as i64),
        None => stack.replace(ctx, Value::Nil),
    }
    Ok(CallbackReturn::Return)
}

fn setreg<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (name, value, cpu): (StdString, i64, Option<StdString>) = stack.consume(ctx)?;
    let id = self::cpu_id(host, ctx, cpu.as_deref())?;
    let applied = host
        .with_core(|core| core.write_register(&id, &name, value as u32))
        .ok_or_else(|| self::detached(ctx))?;

    stack.replace(ctx, applied);
    Ok(CallbackReturn::Return)
}

fn registers<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let cpu: Option<StdString> = stack.consume(ctx)?;
    let id = self::cpu_id(host, ctx, cpu.as_deref())?;
    let view = host
        .with_core(|core| core.cpu_view(&id))
        .ok_or_else(|| self::detached(ctx))?;

    let table = Table::new(&ctx);
    for register in &view.registers {
        table
            .set(
                ctx,
                ctx.intern(register.name.as_bytes()),
                register.value as i64,
            )
            .ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn flags<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let cpu: Option<StdString> = stack.consume(ctx)?;
    let id = self::cpu_id(host, ctx, cpu.as_deref())?;
    let view = host
        .with_core(|core| core.cpu_view(&id))
        .ok_or_else(|| self::detached(ctx))?;

    let table = Table::new(&ctx);
    for flag in &view.flags {
        table
            .set(ctx, ctx.intern(flag.name.as_bytes()), flag.set)
            .ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn on_exec<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::add_access_hook(BreakpointKind::Execute, host, ctx, stack)
}

fn on_read<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::add_access_hook(BreakpointKind::Read, host, ctx, stack)
}

fn on_write<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::add_access_hook(BreakpointKind::Write, host, ctx, stack)
}

fn add_access_hook<'gc>(
    kind: BreakpointKind,
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let args: Vec<Value<'gc>> = stack.drain(..).collect();
    let mut next = 0;

    let start = self::number_at(ctx, &args, &mut next)
        .ok_or_else(|| self::fail(ctx, "hook needs an address"))?;
    let end = self::number_at(ctx, &args, &mut next).unwrap_or(start);

    let function: Function<'gc> = args
        .get(next)
        .copied()
        .ok_or_else(|| self::fail(ctx, "hook needs a callback function"))
        .and_then(|value| Function::from_value(ctx, value).map_err(Error::from))?;
    next += 1;

    let options = args
        .get(next)
        .copied()
        .and_then(|value| Table::from_value(ctx, value).ok());
    let (cpu, region) = self::hook_options(ctx, options);

    let id = {
        let mut state = host.state();
        let Some(cpu_id) = state.cpu(cpu.as_deref()).map(|cpu| cpu.id.clone()) else {
            return Err(self::fail(
                ctx,
                format!("unknown cpu `{}`", cpu.unwrap_or_default()),
            ));
        };
        let Some(region) = state.region(cpu.as_deref(), region.as_deref()) else {
            return Err(self::fail(
                ctx,
                format!("unknown region `{}`", region.unwrap_or_default()),
            ));
        };
        state.add_hook(
            Trigger::Access(kind),
            start.min(end) as u32,
            start.max(end) as u32,
            cpu_id,
            region,
            ctx.stash(function),
        )
    };

    stack.replace(ctx, id as i64);
    Ok(CallbackReturn::Return)
}

fn on_event<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let args: Vec<Value<'gc>> = stack.drain(..).collect();

    let id = match args.first().copied() {
        Some(Value::String(id)) => StdString::from_utf8_lossy(id.as_bytes()).into_owned(),
        _ => return Err(self::fail(ctx, "sly.on_event needs an event id")),
    };

    let mut next = 1;
    let value = self::number_at(ctx, &args, &mut next).map(|value| value as u32);
    let function: Function<'gc> = args
        .get(next)
        .copied()
        .ok_or_else(|| self::fail(ctx, "sly.on_event needs a callback function"))
        .and_then(|value| Function::from_value(ctx, value).map_err(Error::from))?;

    let id = {
        let state = host.state();
        match state
            .events
            .iter()
            .find(|known| known.eq_ignore_ascii_case(&id))
        {
            Some(known) => known.clone(),
            None => {
                let known: Vec<&str> = state.events.iter().map(|id| &**id).collect();
                return Err(self::fail(
                    ctx,
                    format!("unknown event `{id}`, this core reports {known:?}"),
                ));
            }
        }
    };

    let hook = {
        let mut state = host.state();
        let Some(cpu) = state
            .cpu(None)
            .map(|cpu| (cpu.id.clone(), cpu.space.clone()))
        else {
            return Err(self::fail(ctx, "core exposes no processors"));
        };
        state.add_hook(
            Trigger::Event { id, value },
            0,
            0,
            cpu.0,
            cpu.1,
            ctx.stash(function),
        )
    };

    stack.replace(ctx, hook as i64);
    Ok(CallbackReturn::Return)
}

fn events<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let table = Table::new(&ctx);
    for (ix, event) in host.state().events.iter().enumerate() {
        table
            .set(ctx, ix as i64 + 1, ctx.intern(event.as_bytes()))
            .ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn find<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    const CHUNK: usize = 0x4000;
    const DEFAULT_LIMIT: usize = 256;

    let (pattern, options): (Value<'gc>, Option<Table<'gc>>) = stack.consume(ctx)?;

    let needle = match pattern {
        Value::String(text) => text.as_bytes().to_vec(),
        Value::Table(bytes) => (1..=bytes.length())
            .map(|ix| self::as_integer(bytes.get(ctx, ix)) as u8)
            .collect(),
        _ => return Err(self::fail(ctx, "sly.find needs a string or byte table")),
    };

    if needle.is_empty() {
        return Err(self::fail(ctx, "sly.find needs a non-empty pattern"));
    }
    if needle.len() > CHUNK {
        return Err(self::fail(
            ctx,
            format!("sly.find patterns are limited to {CHUNK} bytes"),
        ));
    }

    let field = |key: &'static str| {
        options.and_then(|options| match options.get(ctx, key) {
            Value::Nil => None,
            value => Some(value),
        })
    };
    let region = match field("region") {
        Some(Value::String(region)) => {
            Some(StdString::from_utf8_lossy(region.as_bytes()).into_owned())
        }
        _ => None,
    };
    let start = field("start").map_or(0, |value| self::as_integer(value) as u32);
    let limit = field("limit").map_or(DEFAULT_LIMIT, |value| {
        (self::as_integer(value).max(1) as usize).min(4096)
    });

    let region = self::resolve_region(host, ctx, region.as_deref())?;
    let end = host.state().region_size(&region).unwrap_or(0);
    if end == 0 {
        return Err(self::fail(
            ctx,
            format!("region `{region}` has no searchable size"),
        ));
    }

    let mut found = Vec::new();
    let mut addr = start.min(end);
    let overlap = needle.len() as u32 - 1;

    while addr < end && found.len() < limit {
        let span = CHUNK.min((end - addr) as usize);
        let Some(bytes) = host.with_core(|core| core.read_memory(&region, addr, span)) else {
            return Err(self::detached(ctx));
        };

        for offset in 0..bytes.len().saturating_sub(needle.len() - 1) {
            if bytes[offset..offset + needle.len()] == needle[..] {
                found.push(addr + offset as u32);
                if found.len() >= limit {
                    break;
                }
            }
        }

        if span < CHUNK {
            break;
        }
        addr += CHUNK as u32 - overlap;
    }

    let table = Table::new(&ctx);
    for (ix, hit) in found.iter().enumerate() {
        table.set(ctx, ix as i64 + 1, *hit as i64).ok();
    }

    stack.replace(ctx, table);
    Ok(CallbackReturn::Return)
}

fn dump<'gc>(
    _host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let value: Value<'gc> = stack.consume(ctx)?;
    let text = self::render(ctx, value, DUMP_DEPTH);

    stack.replace(ctx, ctx.intern(text.as_bytes()));
    Ok(CallbackReturn::Return)
}

fn on_frame<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let function: Function<'gc> = stack.consume(ctx)?;

    let id = {
        let mut state = host.state();
        let Some(cpu) = state
            .cpu(None)
            .map(|cpu| (cpu.id.clone(), cpu.space.clone()))
        else {
            return Err(self::fail(ctx, "core exposes no processors"));
        };
        state.add_hook(Trigger::Frame, 0, 0, cpu.0, cpu.1, ctx.stash(function))
    };

    stack.replace(ctx, id as i64);
    Ok(CallbackReturn::Return)
}

fn remove<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let id: i64 = stack.consume(ctx)?;
    let removed = host.state().remove_hook(id.max(0) as u64);

    stack.replace(ctx, removed);
    Ok(CallbackReturn::Return)
}

fn clear_hooks<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let mut state = host.state();
    let count = state.hooks.len();
    state.hooks.clear();
    drop(state);

    stack.replace(ctx, count as i64);
    Ok(CallbackReturn::Return)
}

fn pause<'gc>(
    host: &Host,
    _ctx: Context<'gc>,
    _stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    host.state().pause = Some(true);
    Ok(CallbackReturn::Return)
}

fn resume<'gc>(
    host: &Host,
    _ctx: Context<'gc>,
    _stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    host.state().pause = Some(false);
    Ok(CallbackReturn::Return)
}

fn button<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (name, pressed, player): (StdString, bool, Option<i64>) = stack.consume(ctx)?;
    let button = self::button_from_name(&name)
        .ok_or_else(|| self::fail(ctx, format!("unknown button `{name}`")))?;

    host.with_core(|core| core.set_button(player.unwrap_or(0).max(0) as usize, button, pressed))
        .ok_or_else(|| self::detached(ctx))?;

    Ok(CallbackReturn::Return)
}

fn button_from_name(name: &str) -> Option<Button> {
    Button::ALL
        .into_iter()
        .find(|button| format!("{button:?}").eq_ignore_ascii_case(name))
}

fn frame<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let frame = host.state().frame as i64;
    stack.replace(ctx, frame);
    Ok(CallbackReturn::Return)
}

fn status<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let text = self::joined(ctx, stack);
    host.state().status = (!text.is_empty()).then(|| text.into());
    Ok(CallbackReturn::Return)
}

fn print_at<'gc>(
    level: ConsoleLevel,
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    host.print(level, self::joined(ctx, stack));
    Ok(CallbackReturn::Return)
}

fn log<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::print_at(ConsoleLevel::Print, host, ctx, stack)
}

fn warn<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::print_at(ConsoleLevel::Warn, host, ctx, stack)
}

fn error_line<'gc>(
    host: &Host,
    ctx: Context<'gc>,
    stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    self::print_at(ConsoleLevel::Error, host, ctx, stack)
}

fn hex<'gc>(
    _host: &Host,
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    let (value, digits): (i64, Option<i64>) = stack.consume(ctx)?;
    let width = digits.unwrap_or(2).clamp(1, 16) as usize;
    let text = format!("${:0width$X}", value as u64, width = width);

    stack.replace(ctx, ctx.intern(text.as_bytes()));
    Ok(CallbackReturn::Return)
}

fn number_at<'gc>(ctx: Context<'gc>, args: &[Value<'gc>], next: &mut usize) -> Option<i64> {
    let value = args.get(*next).copied()?;
    if !matches!(value, Value::Integer(_) | Value::Number(_)) {
        return None;
    }

    let number = i64::from_value(ctx, value).ok()?;
    *next += 1;
    Some(number)
}

fn hook_options<'gc>(
    ctx: Context<'gc>,
    options: Option<Table<'gc>>,
) -> (Option<StdString>, Option<StdString>) {
    let Some(options) = options else {
        return (None, None);
    };

    let field = |key: &'static str| match options.get(ctx, key) {
        Value::String(text) => Some(StdString::from_utf8_lossy(text.as_bytes()).into_owned()),
        _ => None,
    };
    (field("cpu"), field("region"))
}
