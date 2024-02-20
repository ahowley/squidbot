use std::str::Chars;

use rand::Rng;

fn roll_dice(number: u32, faces: u32) -> Vec<u32> {
    let mut results: Vec<u32> = Vec::with_capacity(number as usize);
    for _ in 0..number {
        results.push(rand::thread_rng().gen_range(1..faces + 1));
    }

    results
}

fn get_precedence(symbol: char) -> Option<i32> {
    match symbol {
        '+' => Some(1),
        '-' => Some(1),
        '*' => Some(2),
        '/' => Some(2),
        '^' => Some(3),
        'd' => Some(4),
        _ => None,
    }
}

fn evaluate(value1: f64, value2: f64, op: char) -> Option<f64> {
    match op {
        '+' => Some(value1 + value2),
        '-' => Some(value1 - value2),
        '*' => Some(value1 * value2),
        '/' => {
            if value2 == 0. {
                None
            } else {
                Some(value1 / value2)
            }
        }
        '^' => {
            if value2 > 0. && value2 < u32::MAX.into() {
                if let Some(_) = i64::checked_pow(value1.ceil() as i64, value2.ceil() as u32) {
                    Some(f64::powf(value1, value2))
                } else {
                    None
                }
            } else {
                None
            }
        }
        'd' => {
            if value2.abs().round() == 1. {
                Some(value1 * value2)
            } else if value1.round() > u32::MAX.into()
                || value2.round() > u32::MAX.into()
                || value1.round() < 0.
                || value2.round() <= 0.
            {
                None
            } else {
                Some(
                    roll_dice(value1.round() as u32, value2.round() as u32)
                        .into_iter()
                        .map(|val| val as u64)
                        .sum::<u64>() as f64,
                )
            }
        }
        _ => None,
    }
}

fn recursive_descent(start_value: f64, operator: char, expr: &mut Chars<'_>) -> Option<f64> {
    let mut op = operator;
    let mut current_precedence = get_precedence(op)?;
    let mut first_value = start_value;
    let mut parsing_value = String::from("");
    let mut need_op_after_parenthesis = false;
    while let Some(symbol) = expr.next() {
        if symbol == ' ' {
            continue;
        }

        if symbol == '(' {
            let mut nested_value = recursive_descent(0., '+', expr)?;
            if parsing_value.len() > 0 {
                nested_value = evaluate(parsing_value.parse::<f64>().ok()?, nested_value, '*')?;
            }

            parsing_value = format!("{}", nested_value);
            need_op_after_parenthesis = true;
            continue;
        }

        if symbol == ')' {
            return evaluate(first_value, parsing_value.parse::<f64>().ok()?, op);
        }

        if let Some(next_precedence) = get_precedence(symbol) {
            need_op_after_parenthesis = false;
            if parsing_value.len() == 0 {
                return None;
            }

            if next_precedence > current_precedence {
                let parsed_value =
                    recursive_descent(parsing_value.parse::<f64>().ok()?, symbol, expr)?;
                return evaluate(first_value, parsed_value, op);
            }

            first_value = evaluate(first_value, parsing_value.parse::<f64>().ok()?, op)?;
            parsing_value.drain(..);
            op = symbol;
            current_precedence = get_precedence(op)?;
            continue;
        }

        if symbol.to_digit(10).is_none() && symbol != '.' {
            continue;
        }

        if need_op_after_parenthesis {
            return None;
        }

        parsing_value.push(symbol);
    }

    evaluate(first_value, parsing_value.parse::<f64>().ok()?, op)
}

pub fn num_with_thousands_commas(num: u64) -> String {
    num.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(",")
}

pub fn dicemath(expr: &str) -> Option<f64> {
    let result = recursive_descent(0., '+', &mut expr.chars())?;

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_add() {
        assert_eq!(evaluate(1., 1., '+').unwrap(), 2.);
        assert_eq!(evaluate(1., -1., '+').unwrap(), 0.);
        assert_eq!(evaluate(1., 2.5, '+').unwrap(), 3.5);
    }

    #[test]
    fn eval_sub() {
        assert_eq!(evaluate(1., 1., '-').unwrap(), 0.);
        assert_eq!(evaluate(1., -1., '-').unwrap(), 2.);
        assert_eq!(evaluate(1., 2.5, '-').unwrap(), -1.5);
    }

    #[test]
    fn eval_mul() {
        assert_eq!(evaluate(1., 1., '*').unwrap(), 1.);
        assert_eq!(evaluate(1., -1., '*').unwrap(), -1.);
        assert_eq!(evaluate(1., 2.5, '*').unwrap(), 2.5);
    }

    #[test]
    fn eval_div() {
        assert_eq!(evaluate(1., 1., '/').unwrap(), 1.);
        assert_eq!(evaluate(1., -1., '/').unwrap(), -1.);
        assert_eq!(evaluate(10., 2., '/').unwrap(), 5.);
    }

    #[test]
    fn eval_exp() {
        assert_eq!(evaluate(1., 1., '^').unwrap(), 1.);
        assert_eq!(evaluate(1., -1., '^').unwrap_or(0.), 0.);
        assert_eq!(evaluate(10., 2., '^').unwrap(), 100.);
    }

    #[test]
    fn eval_roll() {
        assert_eq!(evaluate(1., 1., 'd').unwrap(), 1.);
        assert_eq!(evaluate(2., 1., 'd').unwrap(), 2.);
        assert!(evaluate(2., 0., 'd').is_none());
    }

    #[test]
    fn parse_flat() {
        assert!(recursive_descent(0., '+', &mut "1 ++ 1".chars()).is_none());
        assert_eq!(
            recursive_descent(0., '+', &mut "1 + 1".chars()).unwrap(),
            2.
        );
        assert_eq!(
            recursive_descent(0., '+', &mut "1 * 2 + 2".chars()).unwrap(),
            4.
        );
        assert_eq!(
            recursive_descent(0., '+', &mut "1 + 2.5 * 2".chars()).unwrap(),
            6.
        );
        assert_eq!(
            recursive_descent(0., '+', &mut "1 + 2 * 2 - 3 ^ 2".chars()).unwrap(),
            -4.
        );
        assert_eq!(
            recursive_descent(
                0.,
                '+',
                &mut " 1 + as2 * 2 vaagmt- 3 maDSGbW$$$^ 2DV vv Wwq    ".chars()
            )
            .unwrap(),
            -4.
        );
    }

    #[test]
    fn parse_nested() {
        assert_eq!(
            recursive_descent(0., '+', &mut "(1 + 2) * 2 - 3 ^ 2".chars()).unwrap(),
            -3.
        );
        assert_eq!(
            recursive_descent(0., '+', &mut "1 + (2 * 2 - 3) ^ 2".chars()).unwrap(),
            2.
        );
        assert_eq!(
            recursive_descent(0., '+', &mut "1 + (2 * 2 - 3) ^ 2 * 3 + 1 / 2".chars()).unwrap(),
            4.5
        );
        assert_eq!(
            recursive_descent(0., '+', &mut "1 + (2 * 2 - 1) ^ (2 * (3 + 1) / 2)".chars()).unwrap(),
            82.
        );
        assert!([5., 7., 9.]
            .contains(&recursive_descent(0., '+', &mut "1 + 2d(2.3 / 1) * (2)".chars()).unwrap()));
    }
}
