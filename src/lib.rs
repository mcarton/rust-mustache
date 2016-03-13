#![crate_name = "mustache"]

#![crate_type = "dylib"]
#![crate_type = "rlib"]

#![allow(unused_attributes)]

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

// mustache has lots of those for historical reason
#![cfg_attr(feature="clippy", allow(str_to_string))]
// the convention is to use `iter`
#![cfg_attr(feature="clippy", allow(explicit_iter_loop))]
// has false positives
#![cfg_attr(feature="clippy", allow(or_fun_call))]

extern crate rustc_serialize;
extern crate log;
#[cfg(test)]extern crate tempdir;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::str;
use std::path::{PathBuf, Path};

pub use self::Data::*;
pub use builder::{MapBuilder, VecBuilder};
pub use encoder::{Error, IoError, InvalidStr, Encoder, EncoderResult};
pub use template::Template;


pub mod builder;
pub mod encoder;

mod compiler;
mod parser;
mod template;

pub enum Data {
    OptVal(Option<Box<Data>>),
    StrVal(String),
    Bool(bool),
    VecVal(Vec<Data>),
    Map(HashMap<String, Data>),
    Fun(RefCell<Box<FnMut(String) -> String + Send>>),
}

impl<'a> PartialEq for Data {
    #[inline]
    fn eq(&self, other: &Data) -> bool {
        match (self, other) {
            (&OptVal(ref v0), &OptVal(ref v1)) => v0 == v1,
            (&StrVal(ref v0), &StrVal(ref v1)) => v0 == v1,
            (&Bool(ref v0), &Bool(ref v1)) => v0 == v1,
            (&VecVal(ref v0), &VecVal(ref v1)) => v0 == v1,
            (&Map(ref v0), &Map(ref v1)) => v0 == v1,
            (&Fun(_), &Fun(_)) => panic!("cannot compare closures"),
            (_, _) => false,
        }
    }
}

impl<'a> fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OptVal(ref v) => write!(f, "OptVal({:?})", v),
            StrVal(ref v) => write!(f, "StrVal({})", v),
            Bool(v) => write!(f, "Bool({:?})", v),
            VecVal(ref v) => write!(f, "VecVal({:?})", v),
            Map(ref v) => write!(f, "Map({:?})", v),
            Fun(_) => write!(f, "Fun(...)"),
        }
    }
}

/// Represents the shared metadata needed to compile and render a mustache
/// template.
#[derive(Clone)]
pub struct Context {
    pub template_path: PathBuf,
    pub template_extension: String,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Context {{ template_path: {:?}, template_extension: {} }}",
               &*self.template_path,
               self.template_extension)
    }
}

impl Context {
    /// Configures a mustache context the specified path to the templates.
    pub fn new(path: PathBuf) -> Context {
        Context {
            template_path: path,
            template_extension: "mustache".to_string(),
        }
    }

    /// Compiles a template from a string
    pub fn compile<IT: Iterator<Item=char>>(&self, reader: IT) -> Template {
        let compiler = compiler::Compiler::new(self.clone(), reader);
        let (tokens, partials) = compiler.compile();

        template::new(self.clone(), tokens, partials)
    }

    /// Compiles a template from a path.
    pub fn compile_path<U: AsRef<Path>>(&self, path: U) -> Result<Template, Error> {
        // FIXME(#6164): This should use the file decoding tools when they are
        // written. For now we'll just read the file and treat it as UTF-8file.
        let mut path = self.template_path.join(path.as_ref());
        path.set_extension(&self.template_extension);
        let mut s = vec![];
        let mut file = try!(File::open(&path));
        try!(file.read_to_end(&mut s));

        // TODO: maybe allow UTF-16 as well?
        let template = match str::from_utf8(&*s) {
            Ok(string) => string,
            _ => { return Result::Err(Error::InvalidStr); }
        };

        Ok(self.compile(template.chars()))
    }
}

/// Compiles a template from an `Iterator<char>`.
pub fn compile_iter<T: Iterator<Item=char>>(iter: T) -> Template {
    Context::new(PathBuf::from(".")).compile(iter)
}

/// Compiles a template from a path.
/// returns None if the file cannot be read OR the file is not UTF-8 encoded
pub fn compile_path<U: AsRef<Path>>(path: U) -> Result<Template, Error> {
    let path = path.as_ref();

    match path.file_name() {
        Some(filename) => {
            let template_dir = path.parent().unwrap_or(Path::new("."));
            // FIXME: Should work with OsStrings, this will not use provided extension if
            // the extension is not utf8 :(
            let extension = path.extension()
                                .and_then(|ext| ext.to_str())
                                .unwrap_or("mustache");

            let context = Context {
                template_path: template_dir.to_path_buf(),
                template_extension: extension.to_string()
            };
            context.compile_path(filename)
        }
        None => Result::Err(Error::NoFilename)
    }
}

/// Compiles a template from a string.
pub fn compile_str(template: &str) -> Template {
    compile_iter(template.chars())
}
