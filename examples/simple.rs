#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_doc_comments)]

// /**@ (x:{x >= 0}) -> v:{v == x + 2} @*/
// fn ipa(x: i32) -> i32 {
//     x + 2
// }

// /**@ (x:{true}) -> v:{v >= 0} @*/
// fn ris(x: i32) -> i32 {
//     if x >= 0 {
//         ipa(x)
//     } else {
//         ipa(0 - x)
//     }
// }

/**@ (n:{n >= 0}) -> v:{v >= n} @*/
#[liquid::ty()]
fn sum(n: u32) -> u32 {
    if n == 0 {
        0
    } else {
        n + sum(n - 1)
    }
}

fn main() {}
