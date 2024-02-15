use crate::ast::{FuncCallArg, Node, Node::*};
use crate::token::Scanner;
use std::fmt::format;

type NodeResult = Result<Box<Node>, String>;

pub struct Parser<'a> {
    scanner: Box<Scanner<'a>>,
}

// shortcuts to go ahead one token
macro_rules! goahead {
    ($parser:ident) => {
        if let Err(err) = $parser.scanner.next_token() {
            return Err(err);
        }
    };
}

impl Parser<'_> {
    pub fn new(input: &str) -> Parser {
        let scanner = Scanner::new(input);
        Parser {
            scanner: Box::new(scanner),
        }
    }

    fn unexpect(&self, expects: &str) -> String {
        format!(
            "unexpected token {}, expect {}",
            self.scanner.unwrap_current_token().kind,
            expects
        )
    }

    fn unexpect_keyword(&self, expects: &str) -> String {
        format!(
            "unexpected keyword {}, expect {}",
            self.scanner.unwrap_current_token().value,
            expects
        )
    }

    pub fn parse(&mut self) -> NodeResult {
        let mut exprs: Vec<Node> = Vec::new();

        goahead!(self);
        while !self.scanner.expect("eof") {
            if self.scanner.expect(";") {
                goahead!(self);
            } else {
                match self.parse_expression() {
                    Ok(node) => exprs.push(*node),
                    Err(err) => return Err(err),
                }
            }
        }
        if exprs.len() == 1 {
            return Ok(Box::new(exprs[0].clone()));
        } else {
            return Ok(Box::new(ExprList { elements: exprs }));
        }
    }

    fn parse_expression(&mut self) -> NodeResult {
        self.parse_in_op()
    }

    // binary operators
    fn parse_binop_keywords(
        &mut self,
        keywords: &[&str],
        sub_func: fn(&mut Self) -> NodeResult,
    ) -> NodeResult {
        let mut left = match sub_func(self) {
            Ok(node) => node,
            Err(err) => return Err(err),
        };

        while self.scanner.expect_keywords(keywords) {
            let op = self.scanner.unwrap_current_token().value;
            goahead!(self);

            let right = match sub_func(self) {
                Ok(node) => node,
                Err(err) => return Err(err),
            };

            left = Box::new(Binop { op, left, right });
        }
        Ok(left)
    }

    fn parse_binop_kinds(
        &mut self,
        kinds: &[&str],
        sub_parse: fn(&mut Self) -> NodeResult,
    ) -> NodeResult {
        let mut left = match sub_parse(self) {
            Ok(node) => node,
            Err(err) => return Err(err),
        };

        while self.scanner.expect_kinds(kinds) {
            let op = self.scanner.unwrap_current_token().value;
            goahead!(self);

            let right = match sub_parse(self) {
                Ok(node) => node,
                Err(err) => return Err(err),
            };

            left = Box::new(Binop { op, left, right });
        }
        Ok(left)
    }

    fn parse_in_op(&mut self) -> NodeResult {
        self.parse_binop_keywords(&["in"], Parser::parse_logic_or)
    }

    fn parse_logic_or(&mut self) -> NodeResult {
        self.parse_binop_keywords(&["or"], Parser::parse_logic_and)
    }

    fn parse_logic_and(&mut self) -> NodeResult {
        self.parse_binop_keywords(&["and"], Parser::parse_compare)
    }

    fn parse_compare(&mut self) -> NodeResult {
        self.parse_binop_kinds(&[">", ">=", "<", "<=", "!=", "="], Parser::parse_add_or_sub)
    }

    fn parse_add_or_sub(&mut self) -> NodeResult {
        self.parse_binop_kinds(&["+", "-"], Parser::parse_mul_or_div)
    }

    fn parse_mul_or_div(&mut self) -> NodeResult {
        self.parse_binop_kinds(&["*", "/", "%"], Parser::parse_funccall_or_index_or_dot)
    }

    fn parse_funccall_or_index_or_dot(&mut self) -> NodeResult {
        let mut node = match self.parse_single_element() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        loop {
            match self.scanner.unwrap_current_token().kind {
                "(" => {
                    node = match self.parse_funccall_rest(node) {
                        Ok(node) => node,
                        Err(err) => return Err(err),
                    };
                }
                "[" => {
                    node = match self.parse_index_rest(node) {
                        Ok(node) => node,
                        Err(err) => return Err(err),
                    };
                }
                "." => {
                    node = match self.parse_dot_rest(node) {
                        Ok(node) => node,
                        Err(err) => return Err(err),
                    };
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_funccall_rest(&mut self, func_node: Box<Node>) -> NodeResult {
        goahead!(self); // skip "("
        let mut args: Vec<FuncCallArg> = Vec::new();
        while !self.scanner.expect(")") {
            match self.parse_funcall_arg() {
                Ok(arg) => {
                    args.push(arg);
                }
                Err(err) => return Err(err),
            };
            if self.scanner.expect(",") {
                goahead!(self);
            } else if !self.scanner.expect(")") {
                return Err(self.unexpect(") and ,"));
            }
        }
        if self.scanner.expect(")") {
            goahead!(self);
        }
        Ok(Box::new(FuncCall {
            func_ref: func_node,
            args,
        }))
    }

    fn parse_funcall_arg(&mut self) -> Result<FuncCallArg, String> {
        let arg = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        if self.scanner.expect(":") {
            goahead!(self);
            if let Var { name } = *arg {
                goahead!(self); // skip ":"
                let arg_value = match self.parse_expression() {
                    Ok(node) => node,
                    Err(err) => return Err(err),
                };
                return Ok(FuncCallArg {
                    arg_name: name,
                    arg: arg_value,
                });
            } else {
                return Err(self.unexpect("'var'"));
            }
        } else {
            return Ok(FuncCallArg {
                arg_name: "".to_owned(),
                arg,
            });
        }
    }

    fn parse_index_rest(&mut self, left: Box<Node>) -> NodeResult {
        goahead!(self); // skip "["

        let at = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        if !self.scanner.expect("]") {
            return Err(self.unexpect("]"));
        }
        goahead!(self);
        return Ok(Box::new(Binop {
            op: "[]".to_owned(),
            left,
            right: at,
        }));
    }

    fn parse_dot_rest(&mut self, left: Box<Node>) -> NodeResult {
        goahead!(self); // skip "."

        let attr = match self.parse_name(None) {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        return Ok(Box::new(DotOp { left, attr }));
    }

    // single element
    fn parse_single_element(&mut self) -> NodeResult {
        match self.scanner.unwrap_current_token().kind {
            "number" => self.parse_number(),
            "name" => self.parse_var(),
            "string" => self.parse_string(),
            "keyword" => match self.scanner.unwrap_current_token().value.as_str() {
                "true" | "false" => self.parse_bool(),
                "null" => self.parse_null(),
                "if" => self.parse_if_expression(),
                "for" => self.parse_for_expression(),
                "some" | "every" => self.parse_some_or_every_expression(),
                _ => return Err(self.unexpect_keyword("true, false")),
            },
            _ => return Err(self.unexpect("name, number")),
        }
    }

    fn parse_name(&mut self, stop_keywords: Option<&[&str]>) -> Result<String, String> {
        let mut names: Vec<String> = Vec::new();

        while self.scanner.expect_kinds(&["name", "keyword"]) {
            let token = self.scanner.unwrap_current_token();
            if let ("keyword", Some(stop_keywords)) = (token.kind, stop_keywords) {
                let token_keyword = token.value.as_str();
                if stop_keywords.into_iter().any(|x| *x == token_keyword) {
                    break;
                }
            }
            names.push(token.value);
            goahead!(self);
        }
        if names.len() > 0 {
            let mut name_buffer = String::new();
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    name_buffer.push_str(" ");
                }
                name_buffer.push_str(name.as_str());
            }
            Ok(name_buffer)
        } else {
            Err(self.unexpect("names"))
        }
    }

    fn parse_var(&mut self) -> NodeResult {
        let token = self.scanner.unwrap_current_token();
        goahead!(self);
        Ok(Box::new(Var { name: token.value }))
    }

    fn parse_number(&mut self) -> NodeResult {
        let token = self.scanner.unwrap_current_token();
        goahead!(self);
        Ok(Box::new(Number { value: token.value }))
    }

    fn parse_string(&mut self) -> NodeResult {
        let token = self.scanner.unwrap_current_token();
        goahead!(self);
        Ok(Box::new(Str { value: token.value }))
    }

    fn parse_bool(&mut self) -> NodeResult {
        let bool_value = match self.scanner.unwrap_current_token().value.as_str() {
            "true" => true,
            "false" => false,
            _ => return Err(self.unexpect_keyword("true, false")),
        };
        goahead!(self);
        Ok(Box::new(Bool { value: bool_value }))
    }

    fn parse_null(&mut self) -> NodeResult {
        goahead!(self); // skip 'null'
        Ok(Box::new(Null))
    }

    // if expression
    fn parse_if_expression(&mut self) -> NodeResult {
        goahead!(self); // skip 'if'
        let cond = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };

        if !self.scanner.expect_keyword("then") {
            return Err(self.unexpect_keyword("then"));
        }
        goahead!(self); // skip 'then'

        let then_branch = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };

        if !self.scanner.expect_keyword("else") {
            return Err(self.unexpect_keyword("else"));
        }
        goahead!(self); // skip 'else'

        let else_branch = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };

        Ok(Box::new(IfExpr {
            condition: cond,
            then_branch,
            else_branch,
        }))
    }

    fn parse_for_expression(&mut self) -> NodeResult {
        goahead!(self); // skip 'for'
        let var_name = match self.parse_name(Some(&["in", "for"])) {
            Ok(var_name) => var_name,
            Err(err) => return Err(err),
        };

        if !self.scanner.expect_keyword("in") {
            return Err(self.unexpect_keyword("in"));
        }
        goahead!(self); // skip 'in'

        let list_expr = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };

        if self.scanner.expect(",") {
            // recursively call for parser
            let return_expr = match self.parse_for_expression() {
                Ok(node) => node,
                Err(err) => return Err(err),
            };
            return Ok(Box::new(ForExpr { var_name, list_expr, return_expr }));
        }
        
        if !self.scanner.expect_keyword("return") {
            return Err(self.unexpect_keyword("return"));
        }
        goahead!(self); // skip 'return'

        let return_expr = match self.parse_for_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        Ok(Box::new(ForExpr { var_name, list_expr, return_expr }))
    }

    fn parse_some_or_every_expression(&mut self) -> NodeResult {
        let cmd = self.scanner.unwrap_current_token().value;
        goahead!(self); // skip 'for'
        let var_name = match self.parse_name(Some(&["in"])) {
            Ok(var_name) => var_name,
            Err(err) => return Err(err),
        };

        if !self.scanner.expect_keyword("in") {
            return Err(self.unexpect_keyword("in"));
        }
        goahead!(self); // skip 'in'

        let list_expr = match self.parse_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        
        if !self.scanner.expect_keyword("satisfies") {
            return Err(self.unexpect_keyword("satisfies"));
        }
        goahead!(self); // skip 'satisfies'

        let filter_expr = match self.parse_for_expression() {
            Ok(node) => node,
            Err(err) => return Err(err),
        };
        if cmd == "some".to_owned() {
            Ok(Box::new(SomeExpr { var_name, list_expr, filter_expr }))
        } else {
            Ok(Box::new(EveryExpr { var_name, list_expr, filter_expr }))
        }
    }

}