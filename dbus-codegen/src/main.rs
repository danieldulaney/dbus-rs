extern crate xml;
extern crate dbus;
extern crate clap;

mod generate;

use dbus::ffidisp::Connection;

use crate::generate::{ServerAccess, ConnectionType};

// Copy-pasted from the output of this program :-)
pub trait OrgFreedesktopDBusIntrospectable {
    fn introspect(&self) -> Result<String, ::dbus::Error>;
}

impl<'a, C: ::std::ops::Deref<Target=::dbus::ffidisp::Connection>> OrgFreedesktopDBusIntrospectable for ::dbus::ffidisp::ConnPath<'a, C> {

    fn introspect(&self) -> Result<String, ::dbus::Error> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Introspectable".into(), &"Introspect".into(), |_| {
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let a0: String = i.read()?;
        Ok(a0)
    }
}


// Unwrapping is fine here, this is just a test program.

fn main() {
    let matches = clap::App::new("D-Bus Rust code generator").about("Generates Rust code from xml introspection data")
        .arg(clap::Arg::with_name("destination").short("d").long("destination").takes_value(true).value_name("BUSNAME")
             .help("If present, connects to the supplied service to get introspection data. Reads from stdin otherwise."))
        .arg(clap::Arg::with_name("path").short("p").long("path").takes_value(true).value_name("PATH")
             .help("The path to ask for introspection data. Defaults to '/'. (Ignored if destination is not specified.)"))
        .arg(clap::Arg::with_name("interfaces").short("f").long("interfaces").takes_value(true).value_name("FILTER")
            .help("Comma separated list of filter strings. Only matching interfaces are generated if set."))
        .arg(clap::Arg::with_name("systembus").short("s").long("system-bus")
             .help("Connects to system bus, if not specified, the session bus will be used. (Ignored if destination is not specified.)"))
        .arg(clap::Arg::with_name("genericvariant").short("g").long("generic-variant")
             .help("If present, will try to make variant arguments generic instead of Variant<Box<dyn RefArg>>. \
Experimental, does not work with server methods (other than None)."))
        .arg(clap::Arg::with_name("methodtype").short("m").long("methodtype").takes_value(true).value_name("Fn")
             .help("Type of server method; valid values are: 'Fn', 'FnMut', 'Sync', 'Generic', and 'None'. Defaults to 'Fn'."))
        .arg(clap::Arg::with_name("methodaccess").short("a").long("methodaccess").takes_value(true).value_name("RefClosure")
             .help("Specifies how to access the type implementing the interface (experimental). Valid values are: 'RefClosure', 'AsRefClosure', 'MethodInfo'. \
Defaults to 'RefClosure'."))
        .arg(clap::Arg::with_name("dbuscrate").long("dbuscrate").takes_value(true).value_name("dbus")
             .help("Name of dbus crate, defaults to 'dbus'."))
        .arg(clap::Arg::with_name("skipprefix").short("i").long("skipprefix").takes_value(true).value_name("PREFIX")
             .help("If present, skips a specific prefix for interface names, e g 'org.freedesktop.DBus.'."))
//        .arg(clap::Arg::with_name("futures").short("f").long("futures")
//             .help("Generates code to use with futures 0.3 (experimental)"))
        .arg(clap::Arg::with_name("client").short("c").long("client").takes_value(true).value_name("client")
             .help("Type of client connection. Valid values are: 'blocking', 'nonblock', 'ffidisp'."))
        .arg(clap::Arg::with_name("output").short("o").long("output").takes_value(true).value_name("FILE")
             .help("Write output into the specified file"))
        .arg(clap::Arg::with_name("file").long("file").required(false).help("D-Bus XML Introspection file"))
        .get_matches();

    if matches.is_present("destination") && matches.is_present("file") {
        panic!("Expected either xml file path as argument or destination option. But both are provided.");
    }

    let s =
    if let Some(dest) = matches.value_of("destination") {
        let path = matches.value_of("path").unwrap_or("/");
        let c = if matches.is_present("systembus") { Connection::new_system() } else { Connection::new_session() };
        let c = c.unwrap();
        let p = c.with_path(dest, path, 10000);
        p.introspect().unwrap()
    } else if let Some(file_path) = matches.value_of("file")  {
        std::fs::read_to_string(file_path.to_string()).unwrap()
    } else {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(),&mut s).unwrap();
        s
    };

    let dbuscrate = matches.value_of("dbuscrate").unwrap_or("dbus");

    let mtype = matches.value_of("methodtype").map(|s| s.to_lowercase());
    let (mtype, crhandler) = match mtype.as_ref().map(|s| &**s) {
        None | Some("fn") => (Some("MTFn"), None),
        Some("fnmut") => (Some("MTFnMut"), None),
        Some("sync") => (Some("MTSync"), None),
        Some("generic") => (Some("MethodType"), None),
        Some("par") => (None, Some("Par")),
        Some("none") => (None, None),
        _ => panic!("Invalid methodtype specified"),
    };

    let maccess = matches.value_of("methodaccess").map(|s| s.to_lowercase());
    let maccess = match maccess.as_ref().map(|s| &**s) {
        None | Some("refclosure") => ServerAccess::RefClosure,
        Some("asrefclosure") => ServerAccess::AsRefClosure,
        Some("methodinfo") => ServerAccess::MethodInfo,
        _ => panic!("Invalid methodaccess specified"),
    };

    let client = matches.value_of("client").map(|s| s.to_lowercase());
    let client = match client.as_ref().map(|s| &**s) {
        None | Some("blocking") => ConnectionType::Blocking,
        Some("nonblock") => ConnectionType::Nonblock,
        Some("ffidisp") => ConnectionType::Ffidisp,
        _ => panic!("Invalid client connection type specified"),
    };

    let interfaces = matches.value_of("interfaces").map(|s| s.split(",").map(|e| e.trim().to_owned()).collect());

    let opts = generate::GenOpts { methodtype: mtype.map(|x| x.into()), dbuscrate: dbuscrate.into(),
        skipprefix: matches.value_of("skipprefix").map(|x| x.into()), serveraccess: maccess,
        genericvariant: matches.is_present("genericvariant"),
        futures: false,
        connectiontype: client,
        crhandler: crhandler.map(|x| x.to_string()),
        interfaces,
        command_line: std::env::args().skip(1).collect::<Vec<String>>().join(" ")
    };

    let mut h: Box<dyn std::io::Write> = match matches.value_of("output") {
        Some(file_path) => Box::new(std::fs::File::create(file_path)
            .unwrap_or_else(|e| {
                panic!("Failed to open {}", e);
            })),
        None => Box::new(std::io::stdout()),
    };

    h.write(generate::generate(&s, &opts).unwrap().as_bytes()).unwrap();
    h.flush().unwrap();
}
