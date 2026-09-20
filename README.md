# 사립 저스티스 학원 열혈청춘일기 2 한국어 패치

PlayStation 일본어판 **사립 저스티스 학원 열혈청춘일기 2**의 비공식 한국어 패치입니다.

현재 버전은 **v0.1.0**입니다.

**[패치 다운로드](https://github.com/mcpads/justice-gakuen2-kr-patch/releases/tag/v0.1.0)** · **[문제 제보](https://github.com/mcpads/justice-gakuen2-kr-patch/issues)**

## 공개 내용

이 저장소는 README와 배포용 xdelta 패치를 제공합니다. 소스 코드와 그 밖의 자료는 추후 공개할 예정입니다. 원본 게임과 패치가 적용된 디스크 이미지는 제공하지 않습니다.

대사·메뉴·그래픽 문구를 한국어로 옮기고, 한글 이름 입력과 표시, 화면별 글꼴 및 숫자 배치를 적용했습니다. 원래 영어로 표시되던 문구는 영어를 유지합니다.

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
2. 릴리스에서 `justice-gakuen2-kr-v0.1.0.xdelta`을 받습니다.
3. [xdelta3](https://github.com/jmacd/xdelta/releases)를 준비하고 원본 BIN의 SHA-256을 위 표와 비교합니다. 배포 패치는 xdelta3 3.2.0으로 생성·재적용 검증했습니다.
4. 아래 명령으로 새 BIN을 만듭니다. `original.bin`은 본인의 원본 BIN 파일명으로 바꾸세요. 경로에 공백이 있으면 큰따옴표로 감쌉니다.

```sh
xdelta3 -d -s "original.bin" "justice-gakuen2-kr-v0.1.0.xdelta" "justice-gakuen2-kr.bin"
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
| xdelta 패치 | 65,968,303바이트 | `0ddd4d98f4089b14f22fe628730b1c7f45b25cc898701da8bc2d1d44cc31d69c` |
| 적용된 BIN | 663,216,960바이트 | `a9f2bd73a9b6e0ff62feb043d8445a693a4fa9a80ffc29c273d4670bba09fddb` |

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

## 0.1.0 주요 변경 사항

- 영문·숫자·기호가 포함된 이름과 별명이 실기시험 준비 및 대전 화면에서 잘못 표시되던 문제를 수정했습니다.
- 시험 결과 화면의 제목과 안내 문구 뒤에 사각형 배경이 드러나던 문제를 수정했습니다.
- 트레이닝에서 캐릭터를 다시 선택한 뒤 공격할 때 멈추던 문제를 수정했습니다.
- 메인 메뉴와 설명, 동아리명, 보너스·열혈비디오 안내 등의 글꼴과 배치를 다듬었습니다.
- 대사와 메뉴의 번역·기호·간격을 보완하고, 카드·편지·이벤트 배경 등의 한국어 그래픽을 개선했습니다.

버전을 바꿀 때는 메모리 카드를 백업하고, 이전 버전의 세이브 스테이트 대신 게임을 새로 부팅해 게임 내 불러오기를 사용하세요. 이전 버전에서 잘못된 이름으로 등록된 EDIT 캐릭터는 다시 등록해야 할 수 있습니다.

## 문제 제보

[Issues](https://github.com/mcpads/justice-gakuen2-kr-patch/issues)에 패치 버전, 에뮬레이터와 버전, 발생한 모드·날짜·장소, 직전 조작을 적어 주세요. 화면 문제가 있다면 스크린샷도 도움이 됩니다. 원본 또는 패치된 게임 이미지는 첨부하지 마세요.

## 제작 및 글꼴

- 한국어 패치: [mcpads](https://github.com/mcpads)
- 원작: CAPCOM
- 사용 글꼴: 갈무리, Neo둥근모, DenkiChip Hangul, MaruMinya Hangul, 달무리, 넥슨 메이플스토리·Lv.2 고딕·풋볼 고딕, 던파 연단된 칼날

게임과 각 글꼴의 권리는 해당 권리자에게 있습니다. 글꼴 파일과 패치 적용 도구는 이 배포에 포함하지 않습니다.
