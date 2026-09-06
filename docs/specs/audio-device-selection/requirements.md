# 要件定義書

## はじめに

gijirec Audio Device Selection は、複数のマイクやスピーカー（出力デバイス）が接続されている環境で Web 会議を利用するユーザーが、OS 既定デバイスに固定されず、使用するマイクとスピーカー（ループバック取得対象）を UI から明示的に選び、その選択で音声キャプチャを行えるようにする機能である。現状の audio-capture は既定デバイスでの二重キャプチャが実装済みだが、デバイス選択 UI は未実装のため、意図しない入力が使われ会議キャプチャの品質や安定性に影響する。本 spec は audio-capture を拡張し、利用可能デバイスの一覧表示、マイク／スピーカー選択、選択デバイスでのキャプチャを提供する。

## スコープ境界

- **対象範囲**: 利用可能なマイク入力デバイスおよびスピーカー（システム音声ループバック取得対象となる出力デバイス）の一覧取得、マイク／スピーカー選択 UI、選択したデバイスでの二重キャプチャ、選択変更時のキャプチャ再開、選択デバイス利用不能時の利用者向け通知、Mac / Windows 上での動作
- **対象外**: 仮想オーディオデバイスの作成・インストール、Linux 対応、デバイスごとの詳細チューニング（ゲイン・イコライザ等）、セッションをまたぐデバイス選択の永続化（アプリ再起動後の前回選択の復元）、Whisper 推論、転写エディタ、Markdown 保存、音声データの外部ネットワーク送信、クラウド音声認識
- **隣接システム・仕様への期待**: 上流の audio-capture が提供する 16 kHz モノラル PCM 出力契約（`docs/contracts/audio-capture-pcm.md`）およびキャプチャフェーズ／エラーイベント契約（`docs/contracts/audio-capture-status.md`）を維持すること。本機能は文字起こし結果やエディタ状態を所有しない。下流 whisper-transcribe は本機能によるデバイス選択後も同一形状の PCM チャンクを受け取れること。アプリのダブルクリック起動・ウィンドウ閉鎖による完全停止のライフサイクルは audio-capture の既存要件と協調する。

## 要件

### 要件 1: 利用可能デバイスの一覧取得

**目的:** 会議利用者として、現在使えるマイクとスピーカーの候補をアプリ内で確認したい。その結果、接続状況に応じて適切なデバイスを選べる。

#### 受け入れ条件

1. When 利用者がデバイス選択 UI を表示する, the gijirec Audio Device Selection shall 現在利用可能なマイク入力デバイスの一覧を取得し、表示する。
2. When 利用者がデバイス選択 UI を表示する, the gijirec Audio Device Selection shall 現在利用可能なスピーカー（システム音声ループバック取得対象となる出力デバイス）の一覧を取得し、表示する。
3. When 一覧内の各デバイスを表示する, the gijirec Audio Device Selection shall 利用者が区別できる表示名（OS が提供するデバイス名または同等の識別情報）を示す。
4. When オーディオデバイスの接続または切断により利用可能デバイスが変化する, the gijirec Audio Device Selection shall デバイス選択 UI が表示されている場合は一覧を自動更新し、利用者が最新の候補を確認できる。
5. If マイクまたはスピーカーの候補が 1 件も存在しない, the gijirec Audio Device Selection shall 該当カテゴリに候補がないことを利用者に示す。

### 要件 2: マイクおよびスピーカーの選択 UI

**目的:** 会議利用者として、使うマイクとスピーカーを画面上で明示的に選びたい。その結果、OS 既定とは異なるデバイスを会議キャプチャに使える。

#### 受け入れ条件

1. The gijirec Audio Device Selection shall マイク用およびスピーカー用の選択 UI を、アプリの既存画面内で利用者が見つけられる場所に提供する。
2. When 利用者がマイク候補のいずれかを選択する, the gijirec Audio Device Selection shall 当該マイクをキャプチャ用の入力デバイスとして記録する。
3. When 利用者がスピーカー候補のいずれかを選択する, the gijirec Audio Device Selection shall 当該出力デバイスをシステム音声（ループバック）取得対象として記録する。
4. While デバイス選択 UI が表示されている, the gijirec Audio Device Selection shall 現在キャプチャに使用しているマイクおよびスピーカーを、選択 UI 上で現在値として示す。
5. When 利用者がまだ明示的に選択を変更していない, the gijirec Audio Device Selection shall OS 既定のマイクおよび既定の出力デバイスを現在値として示す。
6. When 利用者がデバイス選択 UI で明示的な変更を行わない, the gijirec Audio Device Selection shall OS 既定のマイクおよび出力デバイスを用いたキャプチャを、audio-capture の既存ライフサイクル（起動時自動開始）に従って継続する。

### 要件 3: 選択デバイスでの二重キャプチャ

**目的:** 会議利用者として、選んだマイクとスピーカーから音声が取り込まれるようにしたい。その結果、意図した入力で議事録用の PCM が生成される。

#### 受け入れ条件

1. When 利用者がマイクおよびスピーカーの選択を確定する, the gijirec Audio Device Selection shall 記録したマイク入力と記録した出力デバイス（ループバック）の双方から音声を取得する。
2. While 選択済みデバイスでキャプチャが有効である, the gijirec Audio Device Selection shall audio-capture が定義する 16 kHz / 16 bit / モノラル PCM のミックス出力を下流へ連続供給する。
3. When 利用者がマイクまたはスピーカーの選択を変更し確定する, the gijirec Audio Device Selection shall 変更後のデバイスを用いてキャプチャを再開する。
4. While 選択済みデバイスでキャプチャが有効である, the gijirec Audio Device Selection shall 記録したデバイス以外のマイクまたは出力デバイスからサイレントに取得を切り替えない。
5. The gijirec Audio Device Selection shall 仮想オーディオデバイス（BlackHole 等）の作成またはインストールを利用者に要求しない。

### 要件 4: 選択デバイス利用不能・エラー時の挙動

**目的:** 会議利用者として、選んだデバイスが使えない場合に原因と次の行動が分かるようにしたい。その結果、デバイス接続や権限の問題を自己解決できる。

#### 受け入れ条件

1. If 選択したマイクが利用できない, the gijirec Audio Device Selection shall キャプチャを開始できない、または開始後に直ちに停止し、利用者が理解できる形でマイク利用不能であることを通知する。
2. If 選択したスピーカー（ループバック対象）が利用できない, the gijirec Audio Device Selection shall マイクのみの取得にサイレントフォールバックせず、利用者が理解できる形でシステム音声取得不能であることを通知する。
3. If キャプチャ中に選択済みデバイスが切断または権限が失効する, the gijirec Audio Device Selection shall キャプチャを安全に停止し、利用者に再試行または別デバイス選択を促す通知を行う。
4. When デバイス選択に関するエラー通知を表示する, the gijirec Audio Device Selection shall 技術的な内部コードだけでなく、利用者が次に取れる行動（例: 別デバイスの選択、マイク権限の確認）を示す。
5. When 選択デバイスが利用不能である, the gijirec Audio Device Selection shall 利用者が別の利用可能デバイスを選択して再試行できる状態を維持する。

### 要件 5: 会議中のリソース負荷

**目的:** 会議利用者として、デバイス一覧の取得や選択変更が会議本体のパフォーマンスを著しく損なわないようにしたい。その結果、長時間の Web 会議中も他アプリと並行利用できる。

#### 受け入れ条件

1. While デバイス一覧を取得または更新している, the gijirec Audio Device Selection shall 同一マシン上で Web 会議アプリと並行実行されても、Web 会議アプリ側で持続的な音声または映像の途切れ（gijirec のデバイス操作完了後に改善するもの）を引き起こさない。
2. While 選択済みデバイスでキャプチャが有効である, the gijirec Audio Device Selection shall audio-capture の非機能要件（会議並行実行時の性能）を満たす形で動作する。
3. When 利用者がデバイス選択を変更する, the gijirec Audio Device Selection shall キャプチャ再開に要する時間を、利用者が会議の進行を継続できる範囲に抑える（具体な上限は設計フェーズで定義する）。

### 要件 6: 対応プラットフォーム

**目的:** 製品利用者として、使用 OS 上で同等のデバイス選択体験を得たい。その結果、Mac / Windows 双方で意図したデバイスを選んでキャプチャを開始できる。

#### 受け入れ条件

1. Where 実行環境が macOS である, the gijirec Audio Device Selection shall 要件 1〜5 を満たす形でマイクおよびスピーカー（ループバック）の一覧取得・選択・キャプチャを提供する。
2. Where 実行環境が Windows である, the gijirec Audio Device Selection shall 要件 1〜5 を満たす形でマイクおよびスピーカー（ループバック）の一覧取得・選択・キャプチャを提供する。
3. The gijirec Audio Device Selection shall Linux をサポート対象としない。

### 要件 7: 権限・プライバシー

**目的:** 会議利用者として、選択したデバイスからの音声取得に必要な OS 権限が明示され、会議音声が意図せず外部へ送信されないようにしたい。その結果、機密会議でもローカル処理の範囲で安心して利用できる。

#### 受け入れ条件

1. When 選択したマイクまたはスピーカー（ループバック）の取得に OS 権限が必要である, the gijirec Audio Device Selection shall キャプチャ開始前に OS 標準の権限プロンプトを通じて必要な権限を要求する。
2. While 選択済みデバイスでキャプチャが有効である, the gijirec Audio Device Selection shall 取得した音声データをミキシングおよび下流文字起こしへの供給以外の目的で外部ネットワークへ送信しない。
3. The gijirec Audio Device Selection shall デバイス一覧または選択内容を、利用者の明示的な操作なしに外部ネットワークへ送信しない。
4. The gijirec Audio Device Selection shall ユーザー認証・認可機能を提供しない（ローカルデスクトップアプリの単一利用者前提）。
5. If OS がマイクまたはループバック取得に必要な権限を拒否する, the gijirec Audio Device Selection shall 権限不足であることを利用者に通知し、OS 設定での権限付与または別デバイス選択など利用者が次に取れる行動を示す。
