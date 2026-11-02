//! 按当前键盘布局归一，不自行实现输入规则。
use cloudleaf_types::Key;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
pub(crate) fn normalize(w: WPARAM, l: LPARAM) -> Option<Key> {
    if !single_repeat(l) {
        return None;
    }
    // 安全：仅查询当前线程状态，不保存系统指针；缓冲区固定有效。
    unsafe {
        if [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|k| GetKeyState(i32::from(k.0)) < 0)
        {
            return None;
        }
        let key = match w.0 as u16 {
            x if x == VK_BACK.0 => Key::Backspace,
            x if x == VK_SPACE.0 => Key::Space,
            x if x == VK_RETURN.0 => Key::Enter,
            x if x == VK_ESCAPE.0 => Key::Escape,
            x if x == VK_LEFT.0 => Key::Left,
            x if x == VK_RIGHT.0 => Key::Right,
            x if x == VK_UP.0 => Key::Up,
            x if x == VK_DOWN.0 => Key::Down,
            x if x == VK_PRIOR.0 => Key::PageUp,
            x if x == VK_NEXT.0 => Key::PageDown,
            _ => {
                let mut state = [0u8; 256];
                GetKeyboardState(&mut state).ok()?;
                let mut text = [0u16; 8];
                // 标志4保证只读预判不改变死键状态。
                let count = ToUnicodeEx(
                    w.0 as u32,
                    ((l.0 >> 16) & 0xff) as u32,
                    &state,
                    &mut text,
                    4,
                    Some(GetKeyboardLayout(0)),
                );
                if count <= 0 {
                    return None;
                }
                let value = String::from_utf16(&text[..count as usize]).ok()?;
                let mut chars = value.chars();
                let c = chars.next()?;
                if chars.next().is_some() || c.is_control() {
                    return None;
                }
                Key::Char(c)
            }
        };
        Some(key)
    }
}
fn single_repeat(l: LPARAM) -> bool {
    (l.0 as usize & 0xffff) == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeat_bit_is_not_an_event_identity_and_merged_repeats_are_not_partly_consumed() {
        assert!(single_repeat(LPARAM(1)));
        assert!(single_repeat(LPARAM((1 << 30) | 1)));
        assert!(!single_repeat(LPARAM(0)));
        assert!(!single_repeat(LPARAM(2)));
        assert!(!single_repeat(LPARAM((1 << 30) | 0xffff)));
        assert_eq!(normalize(WPARAM(VK_BACK.0 as usize), LPARAM(2)), None);
    }
}
