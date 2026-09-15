# 사립 저스티스 학원 열혈청춘일기 2 한국어 패치

PlayStation 일본어판 **사립 저스티스 학원 열혈청춘일기 2**의 비공식 한국어 패치입니다.

현재 버전은 **v0.1.0-preview.1 — 사전공개 1**입니다. 프리징 문제 등을 제보받기 위한 공개판으로, 모든 분기와 기능의 검증이 끝난 버전은 아니며, 가독성 및 그래픽 관련 업데이트는 되지 않아 계속해서 진행될 에정입니다.

**[패치 다운로드](https://github.com/mcpads/justice-gakuen2-kr-patch/releases/tag/v0.1.0-preview.1)** · **[문제 제보](https://github.com/mcpads/justice-gakuen2-kr-patch/issues)**

## 공개 내용

이 저장소는 README와 배포용 xdelta 패치를 제공합니다. 소스 코드와 그 밖의 자료는 추후 공개할 예정입니다. 원본 게임과 패치가 적용된 디스크 이미지는 제공하지 않습니다.

대사·메뉴·그래픽 문구의 기초 한국어화, 한글 이름 입력과 표시, 화면별 글꼴 및 숫자 배치를 기초적으로 적용했습니다. 0.1.0 버전에서는 원작에 분위기에 맞게 재생성한 형태로 배포할 예정입니다. 원래 영어로 표시되던 문구는 영어를 유지하는 것을 원칙으로 합니다.

## 적용 대상

본인이 보유한 원본에서 준비한 아래 BIN 파일에 적용합니다. 파일 이름이 같아도 해시가 다르면 적용 대상이 아닙니다.

| 항목 | 값 |
| --- | --- |
| 게임 | Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan) |
| 게임 ID | SLPS-02120 |
| 형식 | 단일 트랙 BIN/CUE, MODE2/2352 |
| 원본 BIN 크기 | 663,216,960바이트 |
| 원본 BIN SHA-256 | `62dbc6ca47ec8d9dfbb5797f35d4e720d63e00b5a8e0edd080b149c3ff4f1823` |

다른 지역판, ISO/CHD 파일, 이미 패치된 BIN에는 바로 적용하지 마세요. 먼저 위 형식과 해시에 맞는 원본 BIN을 준비해야 합니다.

## 패치 적용

1. 원본 BIN/CUE와 메모리 카드 파일을 별도로 보관합니다.
2. 릴리스에서 `justice-gakuen2-kr-v0.1.0-preview.1.xdelta`을 받습니다.
3. [xdelta3](https://github.com/jmacd/xdelta/releases)를 준비하고 원본 BIN의 SHA-256을 위 표와 비교합니다. 배포 패치는 xdelta3 3.2.0으로 생성·재적용 검증했습니다.
4. 아래 명령으로 새 BIN을 만듭니다. `original.bin`은 본인의 원본 BIN 파일명으로 바꾸세요. 경로에 공백이 있으면 큰따옴표로 감쌉니다.

```sh
xdelta3 -d -s "original.bin" "justice-gakuen2-kr-v0.1.0-preview.1.xdelta" "justice-gakuen2-kr.bin"
```

5. 결과 BIN의 SHA-256이 아래 값과 일치하는지 확인합니다.
6. 같은 폴더에 `justice-gakuen2-kr.cue`를 텍스트 파일로 만들고 아래 내용을 저장합니다. 에뮬레이터에서 이 CUE를 엽니다.

```cue
FILE "justice-gakuen2-kr.bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
```

### 파일 확인

| 파일 | 크기 | SHA-256 |
| --- | --- | --- |
| xdelta 패치 | 59,768,552바이트 | `87ba75687f763473b84dbecb0ea4922300e098fdfbcff0dabf1a865cef0645ad` |
| 적용된 BIN | 663,216,960바이트 | `9274614fca9a0eb090ded0eb00950cc751fb0f7232975cbc7d9a6718e0c4aca9` |

SHA-256은 다음 명령으로 확인할 수 있습니다. 파일명을 바꾸어 원본·패치·결과를 각각 확인하세요.

```powershell
# Windows PowerShell
Get-FileHash -Algorithm SHA256 "original.bin"
```

```sh
# macOS
shasum -a 256 "original.bin"
# Linux
sha256sum "original.bin"
```

## 확인한 범위와 남은 문제

이번 배포 파일은 원본에 다시 적용했을 때 검증 대상 BIN과 완전히 일치합니다. 해당 BIN에서 최초 부팅, CPU 대전, 필살기와 합체기, 연출 중 일시정지·재개, 대전의 한글 카운터 표시를 확인했습니다.

아직 확인하거나 다듬을 부분이 있습니다.

- 일부 필살기 효과 중 강한 잔상·줄무늬, 일부 메뉴 전환 중 일시적인 화면 깨짐을 조사하고 있습니다. 원본 게임과의 비교가 남아 있습니다.
- 카드·배경의 작은 글씨와 일부 로딩 문구 등에는 일본어가 남아 있거나 판독·검수가 끝나지 않은 부분이 있습니다.
- 모든 분기·엔딩, 모든 캐릭터의 기술·합체기, 2인 대전, 실기 구동과 각종 메모리 카드 오류 상황을 검증한 것은 아닙니다.
- 기존 세이브와의 모든 조합 및 버전 간 호환성은 확인되지 않았습니다. 메모리 카드를 백업하고, 다른 버전의 세이브 스테이트 대신 게임을 새로 부팅해 게임 내 불러오기를 사용하세요.

확인한 대전 구간에서는 프리징·크래시·입력 불능이 발생하지 않았습니다. 이것이 게임 전체나 모든 에뮬레이터에서의 무결함을 뜻하지는 않습니다.

## 문제 제보

[Issues](https://github.com/mcpads/justice-gakuen2-kr-patch/issues)에 패치 버전, 에뮬레이터와 버전, 발생한 모드·날짜·장소, 직전 조작을 적어 주세요. 화면 문제가 있다면 스크린샷도 도움이 됩니다. 원본 또는 패치된 게임 이미지는 첨부하지 마세요.

## 제작 및 글꼴

- 한국어 패치: [mcpads](https://github.com/mcpads)
- 원작: CAPCOM
- 사용 글꼴: 갈무리, Neo둥근모, DenkiChip Hangul, MaruMinya Hangul, 달무리, 넥슨 메이플스토리·Lv.2 고딕·풋볼 고딕, 던파 연단된 칼날

게임과 각 글꼴의 권리는 해당 권리자에게 있습니다. 글꼴 파일과 패치 적용 도구는 이 배포에 포함하지 않습니다.
