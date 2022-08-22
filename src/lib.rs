use rustpython_codegen::{compile, symboltable};
use rustpython_compiler_core::CodeObject;
use rustpython_parser::{
    ast::{fold::Fold, ConstantOptimizer, Location},
    error::ParseErrorType,
    parser,
};

pub use rustpython_codegen::compile::CompileOpts;
pub use rustpython_compiler_core::{BaseError as CompileErrorBody, Mode};

#[derive(Debug, thiserror::Error)]
pub enum CompileErrorType {
    #[error(transparent)]
    Compile(#[from] rustpython_codegen::error::CodegenErrorType),
    #[error(transparent)]
    Parse(#[from] rustpython_parser::error::ParseErrorType),
}

pub type CompileError = rustpython_compiler_core::CompileError<CompileErrorType>;

fn error_from_codegen(
    error: rustpython_codegen::error::CodegenError,
    source: &str,
) -> CompileError {
    let statement = get_statement(source, error.location);
    CompileError {
        body: error.into(),
        statement,
    }
}

fn error_from_parse(error: rustpython_parser::error::ParseError, source: &str) -> CompileError {
    let error: CompileErrorBody<ParseErrorType> = error.into();
    let statement = get_statement(source, error.location);
    CompileError {
        body: error.into(),
        statement,
    }
}

/// Compile a given sourcecode into a bytecode object.
pub fn compile(
    source: &str,
    mode: compile::Mode,
    source_path: String,
    opts: compile::CompileOpts,
) -> Result<CodeObject, CompileError> {
    let parser_mode = match mode {
        compile::Mode::Exec => parser::Mode::Module,
        compile::Mode::Eval => parser::Mode::Expression,
        compile::Mode::Single | compile::Mode::BlockExpr => parser::Mode::Interactive,
    };
    let mut ast = match parser::parse(source, parser_mode, &source_path) {
        Ok(x) => x,
        Err(e) => return Err(error_from_parse(e, source)),
    };
    if opts.optimize > 0 {
        ast = ConstantOptimizer::new()
            .fold_mod(ast)
            .unwrap_or_else(|e| match e {});
    }
    compile::compile_top(&ast, source_path, mode, opts).map_err(|e| error_from_codegen(e, source))
}

pub fn compile_symtable(
    source: &str,
    mode: compile::Mode,
    source_path: &str,
) -> Result<symboltable::SymbolTable, CompileError> {
    let parse_err = |e| error_from_parse(e, source);
    let res = match mode {
        compile::Mode::Exec | compile::Mode::Single | compile::Mode::BlockExpr => {
            let ast = parser::parse_program(source, source_path).map_err(parse_err)?;
            symboltable::SymbolTable::scan_program(&ast)
        }
        compile::Mode::Eval => {
            let expr = parser::parse_expression(source, source_path).map_err(parse_err)?;
            symboltable::SymbolTable::scan_expr(&expr)
        }
    };
    res.map_err(|e| error_from_codegen(e.into_codegen_error(source_path.to_owned()), source))
}

fn get_statement(source: &str, loc: Location) -> Option<String> {
    if loc.column() == 0 || loc.row() == 0 {
        return None;
    }
    let line = source.split('\n').nth(loc.row() - 1)?.to_owned();
    Some(line + "\n")
}
