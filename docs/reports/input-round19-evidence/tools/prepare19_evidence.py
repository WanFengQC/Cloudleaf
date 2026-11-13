"""整理新增证据入口；不读取或复制AppData，不操作窗口。"""
from pathlib import Path
from datetime import datetime, timezone
import json
import re

root = Path(__file__).resolve().parents[1]

def write_once(path, value):
    with path.open('x', encoding='utf-8', newline='\n') as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write('\n')

write_once(root / '.local/round19-ui-final/completion.json', {
    'status': 'partial', 'paused_by_user': True,
    'user_instruction': '先暂停窗口自动化，继续代码和 CI',
    'source': '84187386ea36e8fdb3cf88300c0d35aa86c91cb6',
    'binary_sha256': 'cb751c039598c1738f7159638c6b300c0b2bf80919ffd33b4761cb13ac4d07aa',
    'profile': 'Round19-A',
    'observed': ['独立管理窗', '添加自有词云笺/yun jian并保存generation1',
                 '明确开启学习并保存generation2', '全拼yunjian首选云笺', '简拼yj首选云笺'],
    'not_completed': ['可信Applied后真实学习计数', '窗口重启持久化', '关闭学习再次选择',
                      '实际编辑/停用/删除/导入/导出/恢复/清除按钮闭环', 'Unknown/NotApplied与学习窗口组合'],
    'interference': '外部窗口及焦点丢失已观察；Space尝试后原拼音进入恢复区，未记为中文Applied或学习通过',
    'closed_owned_window_observed': False, 'TSF': False,
    'after_pause': '不再发送窗口输入，也不关闭用户当前窗口；继续代码和CI',
    'private_AppData_read_or_copied': False,
    'recorded_at_utc': datetime.now(timezone.utc).isoformat()
})
write_once(root / '.local/round19-ui-first/completion.json', {
    'status': 'failed_automation_then_panel_corrected',
    'source': '1190d0e4becb13d0e1be87ed7fc90ae4d8fc25ba',
    'failure': '初版owned窗口自动化字段定位误落恢复输入；独立顶层面板修复后另起记录',
    'dictionary_changed': False, 'closed_owned_window_observed': True,
    'scope': '关闭观察来自该次既有记录，不代表后来暂停的窗口已关闭', 'TSF': False
})
write_once(root / '.local/round19-operational-boundaries.json', {
    'preserve_failures': ['缺失用户层API编译失败', '缺失快照/来源API编译失败',
                          '用户偏好上界溢出反例', 'Entries旧Eq兼容编译失败',
                          'Clippy与Shell功能编译反馈', '8418738 Linux Store重开Busy',
                          '独立面板前自动化字段误落及外部焦点干扰'],
    'not_success_evidence': ['错误过滤运行0个测试', 'PowerShell命令引号/工具调用错误',
                             '未命名或未授权外部窗口', '构造Isolated来源的worker测试'],
    'corrections': '按真实过滤重新运行专项和完整回归；不将工具或焦点失败计作产品通过',
    'GUI': '用户明确暂停，后续只进行代码、证据和CI'
})

archive = Path('C:/Users/EDY/.codex/tmp/r19-delivery/archive19.py').read_text(encoding='utf-8')
archive = archive.replace(".local/round19-verified-results.json", ".local/round19-locked-results.json")
archive = archive.replace("'comparison-target'}", "'comparison-target','round19-final-build'}")
archive = archive.replace("tree(TMP/'r19-search','search')", "tree(TMP/'r19-search','search')\ntree(TMP/'r19-engine-perf','engine-host-performance')\ntree(TMP/'r19-final-review','final-review')")
archive = archive.replace("ROOT/'.local/build/release'/name", "ROOT/'.local/round19-final-build/release'/name")
(root / '.local/archive19.py').write_text(archive, encoding='utf-8', newline='\n')
failed = (root / '.local/download_ci19.py').read_text(encoding='utf-8')
failed = failed.replace("run['conclusion']=='success'", "run['conclusion']=='failure'")
failed = failed.replace('下载最终精确CI日志/产物', '保留已失败精确CI日志/仍生成的产物')
(root / '.local/download_failed_ci19.py').write_text(failed, encoding='utf-8', newline='\n')

raw = (root / '.local/round19-runs/locked-04/stdout.txt').read_text(encoding='utf-8')
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw)
counts = dict(zip(['passed', 'failed', 'ignored'], [sum(int(row[i]) for row in rows) for i in range(3)]))
assert counts == {'passed': 281, 'failed': 0, 'ignored': 2}, counts
print(json.dumps({'Rust': counts, 'GUI': 'paused; no window calls', 'archive_tool': str(root / '.local/archive19.py')}, ensure_ascii=False))
