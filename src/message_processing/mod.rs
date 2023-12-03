use std::collections::VecDeque;

use crate::Endmark;

pub(crate) fn substring_utf16(original_string: &str, start_index: usize, end_index: usize) -> String {
    String::from_utf16(&*original_string.chars().take(end_index).skip(start_index).map(|char| char as u16).collect::<Vec<u16>>()).unwrap()
}

pub(crate) fn find_message_end_bound_utf16(input: &str, start_checking_from: usize, check_from_left_to_right: bool, up_to: usize, endmark: &Endmark) -> Option<usize> {
    if input.len() < endmark.string.len() { return None; }
    let escape_endmark = endmark.escape;
    let endmark = endmark.string;
    let desired_endmark_queue = endmark.chars().collect::<Vec<char>>();
    let desired_escape_endmark_queue = escape_endmark.chars().collect::<Vec<char>>();
    let start_checking_from = start_checking_from.max(endmark.len()).min(input.len());
    let up_to = up_to.max(0).min(input.len());

    let mut buffered_endmark = VecDeque::with_capacity(endmark.len());
    let mut buffered_escape_endmark = VecDeque::with_capacity(escape_endmark.len());

    if check_from_left_to_right {
        let mut endmark_iter = input.chars();
        let mut escape_iter = input.chars();

        let starting_to_read_index = start_checking_from.checked_sub(endmark.len().max(escape_endmark.len())).unwrap_or(0);
        let mut character_index = 0;
        while character_index < input.len() {
            character_index += 1;
            let endmark_char = endmark_iter.next();
            let escape_char = escape_iter.next();
            if character_index < starting_to_read_index {
                continue;
            }
            insert_on_queue(&mut buffered_endmark, endmark_char.unwrap(), check_from_left_to_right);
            insert_on_queue(&mut buffered_escape_endmark, escape_char.unwrap(), check_from_left_to_right);
            if character_index < start_checking_from {
                continue;
            }
            let is_endmark = buffered_endmark.eq(&desired_endmark_queue);
            let is_escape_endmark = buffered_escape_endmark.eq(&desired_escape_endmark_queue);
            //println!("From left: {buffered_endmark:?}, {buffered_escape_endmark:?}, {is_endmark}, {is_escape_endmark}");
            let real_index = character_index - endmark.len();
            if is_endmark && !is_escape_endmark {
                return Some(real_index);
            }
            if real_index > up_to {
                return None;
            }
        }
    } else {
        let mut endmark_iter = input.chars().rev();
        let mut escape_iter = input.chars().rev();

        let starting_to_read_index = start_checking_from;
        let mut character_index = input.len();
        loop {
            let endmark_char = endmark_iter.next();
            let mut escape_char = escape_iter.next();
            let mut ignore_escape = escape_char.is_none();
            if character_index > starting_to_read_index {
                if character_index == 0 {
                    return None;
                }
                character_index -= 1;
                continue;
            }
            if character_index == starting_to_read_index {
                let n = escape_endmark.len() - endmark.len();
                if n > 0 {
                    if escape_char.is_none() { break; }
                    insert_on_queue(&mut buffered_escape_endmark, escape_char.unwrap(), check_from_left_to_right);
                    for i in 1..n {
                        let next_char = escape_iter.next();
                        if next_char.is_none() { break; }
                        insert_on_queue(&mut buffered_escape_endmark, next_char.unwrap(), check_from_left_to_right);
                    }
                    escape_char = escape_iter.next();
                    ignore_escape = escape_char.is_none();
                }
            }
            insert_on_queue(&mut buffered_endmark, endmark_char.unwrap(), check_from_left_to_right);
            if escape_char.is_some() {
                insert_on_queue(&mut buffered_escape_endmark, escape_char.unwrap(), check_from_left_to_right);
            }
            let is_endmark = buffered_endmark.eq(&desired_endmark_queue);
            let is_escape_endmark = !ignore_escape && buffered_escape_endmark.eq(&desired_escape_endmark_queue);
            if character_index != 0 {
                character_index -= 1;
            }
            //println!("From right: {buffered_endmark:?}, {buffered_escape_endmark:?}, {is_endmark}, {is_escape_endmark}");
            if is_endmark && !is_escape_endmark {
                return Some(character_index);
            }
            if character_index <= up_to {
                return None;
            }
        }
    }

    None
}

pub(crate) fn insert_on_queue<T>(queue: &mut VecDeque<T>, value: T, front_to_back_order: bool) {
    let remove: fn(&mut VecDeque<T>) -> Option<T> = if front_to_back_order { VecDeque::pop_front } else { VecDeque::pop_back };
    let push: fn(&mut VecDeque<T>, T) = if front_to_back_order { VecDeque::push_back } else { VecDeque::push_front };
    if queue.len() >= queue.capacity() {
        remove(queue);
    }
    push(queue, value);
}


pub(crate) fn find_and_process_messages(input: &mut String, mut start_checking_from: usize, endmark: &Endmark, mut action: impl FnMut(&str, &mut bool)) {
    let mut keep_checking = true;
    while keep_checking {
        let end_of_message_index = find_message_end_bound_utf16(input, start_checking_from, true, input.len(), endmark);
        if end_of_message_index.is_none() { return; }
        let end_of_message_index = end_of_message_index.unwrap();
        let message = substring_utf16(input, 0, end_of_message_index).replace(endmark.escape, endmark.string);
        action(&message, &mut keep_checking);
        *input = substring_utf16(input, end_of_message_index + endmark.string.len(), input.len());
        start_checking_from = 0;
    }
}
