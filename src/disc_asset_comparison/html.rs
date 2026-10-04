use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, Result};

use super::model::{
    ComparisonStatus, DecodedLayerComparison, DiscAssetComparisonReport, DiscRecordComparison,
    EmbeddedTimComparison, FixedPresentationGlyphAtlasFamilyAudit,
    FixedPresentationGlyphAtlasSurfaceAudit, FixedPresentationGlyphSourceAuditReport,
};

pub(super) fn write_fixed_presentation_html_report(
    output_dir: &Path,
    report: &FixedPresentationGlyphSourceAuditReport,
) -> Result<()> {
    let mut html = String::new();
    write!(
        html,
        r#"<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Justice Gakuen 2 고정 표시 글리프 계열 비교</title>
<style>
:root {{ color-scheme:dark; --bg:#10141b; --panel:#171d27; --line:#303949; --muted:#9da9ba; --text:#eef3fa; --changed:#ffb454; --good:#76d59a; --warning:#ffdf7e; }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:var(--bg); color:var(--text); font:14px/1.5 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
header {{ position:sticky; top:0; z-index:10; padding:16px 20px; border-bottom:1px solid var(--line); background:rgba(16,20,27,.96); backdrop-filter:blur(8px); }}
h1 {{ margin:0 0 8px; font-size:22px; }}
.summary,.controls,.badges {{ display:flex; flex-wrap:wrap; gap:8px 14px; align-items:center; }}
.summary span,.note {{ color:var(--muted); }}
.summary strong {{ color:var(--text); }}
.warning {{ margin-top:10px; color:var(--warning); max-width:1200px; }}
.controls {{ margin-top:12px; }}
input,select {{ min-height:34px; color:var(--text); background:#0d1118; border:1px solid var(--line); border-radius:6px; padding:6px 10px; }}
input {{ min-width:min(520px,72vw); }}
main {{ padding:18px; max-width:1900px; margin:auto; }}
.identity,.family {{ margin:0 0 16px; padding:14px; background:var(--panel); border:1px solid var(--line); border-radius:8px; }}
.family.changed {{ border-left:4px solid var(--changed); }}
.family h2 {{ margin:0; font-size:16px; overflow-wrap:anywhere; }}
.badge {{ border:1px solid var(--line); border-radius:999px; padding:2px 8px; color:var(--muted); }}
.badge.changed {{ color:var(--changed); border-color:#765b35; }}
.badge.unchanged {{ color:var(--good); border-color:#35684a; }}
.surface {{ margin-top:12px; padding-top:12px; border-top:1px solid var(--line); }}
.surface h3 {{ margin:0 0 8px; font-size:14px; overflow-wrap:anywhere; }}
.images {{ display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:10px; }}
figure {{ margin:0; min-width:0; }}
figcaption {{ margin-bottom:5px; color:var(--muted); }}
img {{ display:block; max-width:100%; max-height:560px; background:repeating-conic-gradient(#26303d 0 25%,#111720 0 50%) 50%/16px 16px; border:1px solid var(--line); image-rendering:pixelated; }}
.empty {{ min-height:90px; display:grid; place-items:center; color:var(--muted); border:1px dashed var(--line); }}
code {{ color:#c9e9ff; overflow-wrap:anywhere; }}
@media (max-width:900px) {{ .images {{ grid-template-columns:1fr; }} header {{ position:static; }} }}
</style>
</head>
<body>
<header>
  <h1>고정 표시 글리프 후보 계열: 원본 ↔ 패치</h1>
  <div class="summary">
    <span>일본어 대상 <strong>{}</strong></span>
    <span>런타임 관측 픽셀 대상 <strong>{}</strong></span>
    <span>원본/패치 후보 표면 <strong>{}/{}</strong></span>
    <span>원본 TIM 계열 <strong>{}</strong></span>
    <span>복제 계열 <strong>{}</strong></span>
    <a href="report.json">JSON 보고서</a>
  </div>
  <div class="warning">자홍색 테두리는 원본 대사 글리프와 색인 픽셀이 정확히 일치한 위치입니다. 일본어 픽셀이 남았다는 사실만으로 런타임 결함이나 실제 소비처를 뜻하지 않습니다.</div>
  <div class="controls">
    <input id="search" type="search" placeholder="경로·글자 검색 (예: MGBGK, 問題, SIKEN)">
    <select id="status">
      <option value="all">모든 계열</option>
      <option value="changed">변경 표면이 있는 계열</option>
      <option value="unchanged">모든 표면이 동일한 계열</option>
      <option value="duplicated">복제 소비처 계열</option>
    </select>
    <span id="visible-count"></span>
  </div>
</header>
<main>
  <div class="identity">
    <div><strong>원본 BIN</strong> <code>{}</code></div>
    <div><strong>패치 BIN</strong> <code>{}</code></div>
    <div class="note">이 화면은 지원되는 디코드 계층의 4-bpp TIM만 다룹니다. family는 exact source TIM SHA-256으로 묶었으며, 같은 loader나 같은 의미를 쓴다는 증명은 아닙니다.</div>
  </div>
"#,
        report.japanese_script_target_count,
        report.runtime_observed_target_count,
        report.source_presentation_candidate_surface_count,
        report.patched_presentation_candidate_surface_count,
        report.source_presentation_candidate_atlas_family_count,
        report.duplicated_source_presentation_candidate_atlas_family_count,
        report.source_bin_sha256,
        report.patched_bin_sha256,
    )?;

    for family in &report.presentation_candidate_atlas_families {
        write_fixed_presentation_family(&mut html, family)?;
    }

    html.push_str(
        r#"</main>
<script>
const families=[...document.querySelectorAll('.family')];
const search=document.getElementById('search');
const status=document.getElementById('status');
const count=document.getElementById('visible-count');
function applyFilters(){
  const query=search.value.trim().toLowerCase();
  let visible=0;
  for(const family of families){
    const matchesText=!query||family.dataset.search.includes(query);
    const mode=status.value;
    const matchesMode=mode==='all'||family.dataset.status===mode||(mode==='duplicated'&&family.dataset.duplicated==='true');
    const show=matchesText&&matchesMode;
    family.hidden=!show;
    if(show) visible++;
  }
  count.textContent=`${visible}개 계열 표시`;
}
search.addEventListener('input',applyFilters);
status.addEventListener('change',applyFilters);
applyFilters();
</script>
</body>
</html>
"#,
    );

    let path = output_dir.join("index.html");
    std::fs::write(&path, html).with_context(|| format!("failed to write {}", path.display()))
}

fn write_fixed_presentation_family(
    html: &mut String,
    family: &FixedPresentationGlyphAtlasFamilyAudit,
) -> std::fmt::Result {
    let has_changed_surface = family.changed_surface_count > 0
        || family.source_only_surface_count > 0
        || family.patched_only_surface_count > 0;
    let family_status = if has_changed_surface {
        "changed"
    } else {
        "unchanged"
    };
    let search_text = format!(
        "{} {} {}",
        family.family_id,
        family
            .surfaces
            .iter()
            .map(|surface| surface.record_path.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        family.source_texts.join("")
    )
    .to_ascii_lowercase();
    write!(
        html,
        "<article class=\"family {family_status}\" data-search=\"{}\" data-status=\"{family_status}\" data-duplicated=\"{}\">",
        escape_html(&search_text),
        family.source_surface_count > 1,
    )?;
    write!(html, "<h2>{}</h2>", escape_html(&family.family_id))?;
    write!(
        html,
        "<div class=\"badges\"><span class=\"badge {family_status}\">{}</span><span class=\"badge\">표면 {}</span><span class=\"badge\">원본/패치 일치 글리프 {}/{}</span><span class=\"badge\">원본/패치 일치 위치 {}/{}</span><span class=\"badge\">런타임 관측 픽셀 대상 {}</span></div>",
        if has_changed_surface {
            "변경 표면 포함"
        } else {
            "모든 TIM 동일"
        },
        family.surface_count,
        family.source_target_count,
        family.patched_target_count,
        family.source_match_count,
        family.patched_match_count,
        family.source_runtime_observed_target_count,
    )?;
    write!(
        html,
        "<p class=\"note\">정확히 일치한 원본 글자: {}</p>",
        escape_html(&fixed_presentation_text_summary(&family.source_texts))
    )?;
    for surface in &family.surfaces {
        write_fixed_presentation_surface(html, surface)?;
    }
    html.push_str("</article>");
    Ok(())
}

fn write_fixed_presentation_surface(
    html: &mut String,
    surface: &FixedPresentationGlyphAtlasSurfaceAudit,
) -> std::fmt::Result {
    let status = status_name(surface.tim_status);
    write!(
        html,
        "<section class=\"surface\"><h3>{} #{} · TIM @ 0x{:x} <span class=\"badge {status}\">{}</span></h3>",
        escape_html(&surface.record_path),
        escape_html(&surface.decoded_layer_id),
        surface.tim_offset,
        status_label(surface.tim_status),
    )?;
    html.push_str("<div class=\"images\">");
    write_figure(
        html,
        "원본 · 일치 후보 강조",
        surface
            .source
            .as_ref()
            .and_then(|side| side.preview_file.as_deref()),
    )?;
    write_figure(
        html,
        "패치 · 일치 후보 강조",
        surface
            .patched
            .as_ref()
            .and_then(|side| side.preview_file.as_deref()),
    )?;
    html.push_str("</div></section>");
    Ok(())
}

fn fixed_presentation_text_summary(texts: &[String]) -> String {
    const LIMIT: usize = 32;
    let shown = texts
        .iter()
        .take(LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if texts.len() > LIMIT {
        format!("{shown} … (총 {}개)", texts.len())
    } else {
        shown
    }
}

pub(super) fn write_html_report(
    output_dir: &Path,
    report: &DiscAssetComparisonReport,
) -> Result<()> {
    let mut html = String::new();
    write!(
        html,
        r#"<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Justice Gakuen 2 원본·패치 에셋 비교</title>
<style>
:root {{ color-scheme: dark; --bg:#10141b; --panel:#171d27; --line:#303949; --muted:#9da9ba; --text:#eef3fa; --accent:#69d2ff; --changed:#ffb454; --good:#76d59a; --missing:#ff7383; }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:var(--bg); color:var(--text); font:14px/1.5 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
header {{ position:sticky; top:0; z-index:10; padding:16px 20px; border-bottom:1px solid var(--line); background:rgba(16,20,27,.96); backdrop-filter:blur(8px); }}
h1 {{ margin:0 0 8px; font-size:22px; }}
.summary,.controls {{ display:flex; flex-wrap:wrap; gap:8px 16px; align-items:center; }}
.summary span {{ color:var(--muted); }}
.summary strong {{ color:var(--text); }}
.controls {{ margin-top:12px; }}
input,select {{ min-height:34px; color:var(--text); background:#0d1118; border:1px solid var(--line); border-radius:6px; padding:6px 10px; }}
input {{ min-width:min(460px,70vw); }}
main {{ padding:18px; max-width:1800px; margin:auto; }}
.identity {{ margin:0 0 18px; padding:12px; background:var(--panel); border:1px solid var(--line); border-radius:8px; overflow-wrap:anywhere; }}
.record {{ margin:0 0 14px; padding:14px; background:var(--panel); border:1px solid var(--line); border-radius:8px; }}
.record.changed {{ border-left:4px solid var(--changed); }}
.record.source_only,.record.patched_only {{ border-left:4px solid var(--missing); }}
.record h2 {{ margin:0; font-size:16px; overflow-wrap:anywhere; }}
.badges {{ display:flex; flex-wrap:wrap; gap:6px; margin:8px 0; }}
.badge {{ border:1px solid var(--line); border-radius:999px; padding:2px 8px; color:var(--muted); }}
.badge.changed {{ color:var(--changed); border-color:#765b35; }}
.badge.unchanged {{ color:var(--good); border-color:#35684a; }}
.badge.source_only,.badge.patched_only {{ color:var(--missing); border-color:#713b44; }}
.layer {{ margin-top:12px; border-top:1px solid var(--line); padding-top:12px; }}
.layer h3 {{ margin:0 0 8px; font-size:14px; }}
.tim {{ margin:10px 0; padding:10px; background:#10151e; border:1px solid #283140; border-radius:7px; }}
.tim h4 {{ margin:0 0 8px; }}
.images {{ display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:10px; }}
figure {{ margin:0; min-width:0; }}
figcaption {{ margin-bottom:5px; color:var(--muted); }}
img {{ display:block; max-width:100%; max-height:560px; background:repeating-conic-gradient(#26303d 0 25%,#111720 0 50%) 50%/16px 16px; border:1px solid var(--line); image-rendering:pixelated; }}
.empty {{ min-height:90px; display:grid; place-items:center; color:var(--muted); border:1px dashed var(--line); }}
details {{ margin-top:8px; color:var(--muted); }}
code {{ color:#c9e9ff; overflow-wrap:anywhere; }}
.note {{ color:var(--muted); }}
@media (max-width:900px) {{ .images {{ grid-template-columns:1fr; }} header {{ position:static; }} }}
</style>
</head>
<body>
<header>
  <h1>원본 ROM ↔ 패치 ROM 에셋 비교</h1>
  <div class="summary">
    <span>ISO 파일 <strong>{}</strong></span>
    <span>변경 <strong>{}</strong></span>
    <span>동일 <strong>{}</strong></span>
    <span>TIM 비교 <strong>{}</strong></span>
    <span>시각 변경 <strong>{}</strong></span>
    <span>CLUT 변경 <strong>{}</strong></span>
    <span>디코드 이슈 <strong>{}</strong></span>
    <a href="report.json">JSON 보고서</a>
  </div>
  <div class="controls">
    <input id="search" type="search" placeholder="경로 검색 (예: MENU.BIZ, MGBG.TZZ)">
    <select id="status">
      <option value="changed">변경 파일만</option>
      <option value="visual">TIM이 검출된 변경 파일</option>
      <option value="all">전체 파일</option>
      <option value="unchanged">동일 파일만</option>
    </select>
    <span id="visible-count"></span>
  </div>
</header>
<main>
  <div class="identity">
    <div><strong>원본</strong> <code>{}</code></div>
    <div><strong>원본 BIN SHA-256</strong> <code>{}</code></div>
    <div><strong>패치</strong> <code>{}</code></div>
    <div><strong>패치 BIN SHA-256</strong> <code>{}</code></div>
    <div class="note">범위: {} 차이 이미지는 색인 픽셀이 달라진 곳을 자홍색, 원본에만 있는 곳을 빨강, 패치에만 있는 곳을 초록으로 표시합니다.</div>
  </div>
"#,
        report.compared_record_count,
        report.changed_record_count,
        report.unchanged_record_count,
        report.embedded_tim_comparison_count,
        report.visually_changed_tim_count,
        report.changed_clut_tim_count,
        report.decode_issue_count,
        escape_html(&report.source_cue),
        report.source_bin_sha256,
        escape_html(&report.patched_cue),
        report.patched_bin_sha256,
        escape_html(&report.visual_scan_scope),
    )?;

    for record in &report.records {
        write_record(&mut html, record)?;
    }

    html.push_str(
        r#"</main>
<script>
const records=[...document.querySelectorAll('.record')];
const search=document.getElementById('search');
const status=document.getElementById('status');
const count=document.getElementById('visible-count');
function applyFilters(){
  const query=search.value.trim().toLowerCase();
  let visible=0;
  for(const record of records){
    const matchesText=!query||record.dataset.path.includes(query);
    const mode=status.value;
    const matchesMode=mode==='all'||(mode==='changed'&&record.dataset.status!=='unchanged')||record.dataset.status===mode||(mode==='visual'&&record.dataset.visual==='true');
    const show=matchesText&&matchesMode;
    record.hidden=!show;
    if(show) visible++;
  }
  count.textContent=`${visible}개 표시`;
}
search.addEventListener('input',applyFilters);
status.addEventListener('change',applyFilters);
applyFilters();
</script>
</body>
</html>
"#,
    );

    let path = output_dir.join("index.html");
    std::fs::write(&path, html).with_context(|| format!("failed to write {}", path.display()))
}

fn write_record(html: &mut String, record: &DiscRecordComparison) -> std::fmt::Result {
    let status = status_name(record.status);
    let has_visuals = record
        .decoded_layers
        .iter()
        .any(|layer| !layer.embedded_tims.is_empty());
    write!(
        html,
        "<article class=\"record {status}\" data-path=\"{}\" data-status=\"{status}\" data-visual=\"{}\">",
        escape_html(&record.path.to_ascii_lowercase()),
        has_visuals,
    )?;
    write!(html, "<h2>{}</h2>", escape_html(&record.path))?;
    write!(
        html,
        "<div class=\"badges\"><span class=\"badge {status}\">{}</span>",
        status_label(record.status)
    )?;
    if let Some(difference) = &record.stored_difference {
        write!(
            html,
            "<span class=\"badge\">저장 바이트 차이 {} / {}개 범위</span>",
            difference.changed_byte_count, difference.changed_range_count
        )?;
    }
    write!(
        html,
        "<span class=\"badge\">디코드 계층 {}</span><span class=\"badge\">TIM {}</span></div>",
        record.decoded_layers.len(),
        record
            .decoded_layers
            .iter()
            .map(|layer| layer.embedded_tim_comparison_count)
            .sum::<usize>()
    )?;

    if !record.decode_issues.is_empty() {
        html.push_str("<details><summary>디코드 이슈</summary><ul>");
        for issue in &record.decode_issues {
            write!(
                html,
                "<li>{:?} / {}: {}</li>",
                issue.side,
                escape_html(&issue.stage),
                escape_html(&issue.message)
            )?;
        }
        html.push_str("</ul></details>");
    }

    for layer in &record.decoded_layers {
        write_layer(html, layer)?;
    }
    if record.decoded_layers.is_empty() && record.status != ComparisonStatus::Unchanged {
        html.push_str("<p class=\"note\">비교 가능한 디코드 계층이 없습니다.</p>");
    } else if !has_visuals && record.status != ComparisonStatus::Unchanged {
        let detected_tim_count = record
            .decoded_layers
            .iter()
            .map(|layer| layer.embedded_tim_comparison_count)
            .sum::<usize>();
        if detected_tim_count == 0 {
            html.push_str(
                "<p class=\"note\">변경된 디코드 계층에서 지원 TIM을 찾지 못했습니다.</p>",
            );
        } else {
            html.push_str("<p class=\"note\">검출된 TIM은 모두 원본과 바이트가 같아 중복 렌더링하지 않았습니다.</p>");
        }
    }
    html.push_str("</article>");
    Ok(())
}

fn write_layer(html: &mut String, layer: &DecodedLayerComparison) -> std::fmt::Result {
    let status = status_name(layer.status);
    write!(
        html,
        "<section class=\"layer\"><h3>{} · {} <span class=\"badge {status}\">{}</span></h3>",
        escape_html(&layer.id),
        escape_html(&layer.kind),
        status_label(layer.status),
    )?;
    if let Some(difference) = &layer.decoded_difference {
        write!(
            html,
            "<div class=\"note\">디코드 바이트 차이 {} / {}개 범위</div>",
            difference.changed_byte_count, difference.changed_range_count
        )?;
    }
    if layer.embedded_tim_scan_performed {
        write!(
            html,
            "<div class=\"note\">TIM 원본 {} / 패치 {} / 합집합 {} / 동일 {} / 렌더링 {}</div>",
            layer.source_embedded_tim_count,
            layer.patched_embedded_tim_count,
            layer.embedded_tim_comparison_count,
            layer.unchanged_embedded_tim_count,
            layer.embedded_tims.len(),
        )?;
    }
    for tim in &layer.embedded_tims {
        write_tim(html, tim)?;
    }
    html.push_str("</section>");
    Ok(())
}

fn write_tim(html: &mut String, tim: &EmbeddedTimComparison) -> std::fmt::Result {
    let status = status_name(tim.status);
    write!(
        html,
        "<div class=\"tim\"><h4>TIM @ 0x{:x} <span class=\"badge {status}\">{}</span></h4>",
        tim.offset,
        status_label(tim.status),
    )?;
    html.push_str("<div class=\"images\">");
    let source_label = if tim.preview_palette_index.is_some() {
        "원본 · 미리보기 팔레트 0"
    } else {
        "원본 · 색인 명암 미리보기"
    };
    let patched_label = if tim.preview_palette_index.is_some() {
        "패치 · 미리보기 팔레트 0"
    } else {
        "패치 · 색인 명암 미리보기"
    };
    write_figure(html, source_label, tim.source_preview_file.as_deref())?;
    write_figure(html, patched_label, tim.patched_preview_file.as_deref())?;
    write_figure(html, "차이", tim.difference_preview_file.as_deref())?;
    html.push_str("</div><div class=\"badges\">");
    if let Some(count) = tim.indexed_pixel_difference_count {
        write!(html, "<span class=\"badge\">색인 픽셀 차이 {count}</span>")?;
    }
    if let Some(count) = tim.clut_word_difference_count {
        write!(
            html,
            "<span class=\"badge\">전체 CLUT 단어 차이 {count}</span>"
        )?;
    }
    if let Some(count) = tim.palette_zero_rgba_difference_count {
        write!(
            html,
            "<span class=\"badge\">미리보기 팔레트 0 적용 RGBA 차이 {count}</span>"
        )?;
    }
    if let Some(difference) = &tim.tim_byte_difference {
        write!(
            html,
            "<span class=\"badge\">TIM 바이트 차이 {}</span>",
            difference.changed_byte_count
        )?;
    }
    html.push_str("</div>");
    if let Some(error) = &tim.preview_error {
        write!(
            html,
            "<div class=\"note\">미리보기 오류: {}</div>",
            escape_html(error)
        )?;
    }
    html.push_str("</div>");
    Ok(())
}

fn write_figure(html: &mut String, label: &str, path: Option<&str>) -> std::fmt::Result {
    write!(html, "<figure><figcaption>{label}</figcaption>")?;
    if let Some(path) = path {
        write!(
            html,
            "<a href=\"{}\"><img loading=\"lazy\" src=\"{}\" alt=\"{}\"></a>",
            escape_html(path),
            escape_html(path),
            label
        )?;
    } else {
        html.push_str("<div class=\"empty\">없음</div>");
    }
    html.push_str("</figure>");
    Ok(())
}

fn status_name(status: ComparisonStatus) -> &'static str {
    match status {
        ComparisonStatus::Unchanged => "unchanged",
        ComparisonStatus::Changed => "changed",
        ComparisonStatus::SourceOnly => "source_only",
        ComparisonStatus::PatchedOnly => "patched_only",
    }
}

fn status_label(status: ComparisonStatus) -> &'static str {
    match status {
        ComparisonStatus::Unchanged => "동일",
        ComparisonStatus::Changed => "변경",
        ComparisonStatus::SourceOnly => "원본에만 존재",
        ComparisonStatus::PatchedOnly => "패치에만 존재",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::escape_html;

    #[test]
    fn escapes_paths_before_writing_html_attributes() {
        assert_eq!(
            escape_html("A&B/<asset>\"'"),
            "A&amp;B/&lt;asset&gt;&quot;&#39;"
        );
    }
}
