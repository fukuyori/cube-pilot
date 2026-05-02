# Cube Pilot

Macroquad 製の 3D ルービックキューブ表示・操作アプリです。

[English](README.md)

バージョン: 0.6.0

## 実行画面

![Cube Pilot running screen](docs/images/cube-pilot-screen.png)

## 実行方法

```powershell
cargo run
```

## キーアサイン

画面上部にはコンパクトなメニューバーとツールバーがあります。
ツールバーボタンは、シャッフル/停止、自動シャッフル、ソルバー、
解法ステップ操作、リセット、視点リセット、正面ラベル、ピン消去に対応しています。

### メニュー・ツールバー・ショートカット

| メニュー | ツールバー | ショートカット | 操作 |
| --- | --- | --- | --- |
| Game > Scramble / Stop | Scr | Space | 停止中は通常シャッフルを開始し、アニメーション中または自動シャッフル中は停止する |
| Game > Auto Shuffle | Auto | Shift + A | ゆっくりした自動シャッフルを切り替える。動作中はツールバーボタンが強調表示される |
| Game > Solve | Sol | S | 停止中に現在のキューブを解く |
| Help > Solution Prev | < | Left Arrow | 直前の解法ステップを戻す |
| Help > Solution Next | > | Right Arrow | 次の解法ステップを再生する |
| Help > Solution Play | >> | Down Arrow | 残りの解法ステップをすべて再生する |
| Game > Reset | Reset | Shift + R | キューブと待機中のアニメーションをリセットする |
| View > Reset View | View | Numpad 5 | カメラを初期視点に戻す |
| View > Front Labels | Face | F | 現在の正面を点滅表示し、テンキー位置ラベルを表示する |
| View > Clear Pins | Clr | - | すべてのピンを消す |
| Game > Quit | - | Esc | アプリを終了する |

### キューブ回転

これらの操作は現在の視点を基準にします。
正面の面はテンキーのようにクリックできます。7/4/1 の位置をクリックすると行を左回転し、
9/6/3 の位置をクリックすると行を右回転します。Shift を押しながら 1/2/3 または
7/8/9 の位置をクリックすると列を縦回転します。

| キー / 入力 | 操作 |
| --- | --- |
| Numpad 7 | 見えている上段を左回転する |
| Numpad 4 | 見えている中段を左回転する |
| Numpad 1 | 見えている下段を左回転する |
| Numpad 9 | 見えている上段を右回転する |
| Numpad 6 | 見えている中段を右回転する |
| Numpad 3 | 見えている下段を右回転する |
| Shift + Numpad 1 | 見えている左列を前方向に回転する |
| Shift + Numpad 2 | 見えている中央列を前方向に回転する |
| Shift + Numpad 3 | 見えている右列を前方向に回転する |
| Shift + Numpad 7 | 見えている左列を後方向に回転する |
| Shift + Numpad 8 | 見えている中央列を後方向に回転する |
| Shift + Numpad 9 | 見えている右列を後方向に回転する |
| 正面 7/4/1 位置を Left Click | その行を左回転する |
| 正面 9/6/3 位置を Left Click | その行を右回転する |
| 正面 1/2/3 位置を Shift + Left Click | その列を前方向に回転する |
| 正面 7/8/9 位置を Shift + Left Click | その列を後方向に回転する |

### シャッフルとソルバー

| キー | 操作 |
| --- | --- |
| Space | 停止中は通常シャッフルを開始し、アニメーション中または自動シャッフル中は停止する |
| Shift + A | ゆっくりした自動シャッフルを切り替える。停止するまでゆっくりしたシャッフル手順を追加し続ける |
| S | 停止中に現在のキューブを解く |
| Right Arrow | 次の解法ステップを再生する |
| Left Arrow | 直前の解法ステップを戻す |
| Down Arrow | 残りの解法ステップをすべて再生する |
| Shift + R | キューブと待機中のアニメーションをリセットする |

### 視点と表示

| キー / 入力 | 操作 |
| --- | --- |
| Mouse drag | カメラを回り込ませる |
| Ctrl + Numpad 7 または Ctrl + Numpad 4 | カメラを左にヨー回転する |
| Ctrl + Numpad 9 または Ctrl + Numpad 6 | カメラを右にヨー回転する |
| Ctrl + Numpad 8 | カメラを上にピッチ回転する |
| Ctrl + Numpad 2 | カメラを下にピッチ回転する |
| 正面 7/4 位置を Ctrl + Left Click | カメラを左にヨー回転する |
| 正面 9/6 位置を Ctrl + Left Click | カメラを右にヨー回転する |
| 正面 8 位置を Ctrl + Left Click | カメラを上にピッチ回転する |
| 正面 2 位置を Ctrl + Left Click | カメラを下にピッチ回転する |
| Numpad 5 | カメラを初期視点に戻す |
| F | 現在の正面を点滅表示し、テンキー位置ラベルを表示する |
| F + Left Click | クリックした正面ステッカーに、消すまで残るピンを刺す |
| Esc | アプリを終了する |
