# inZOI Better Mod Manager

Tôi đã bị mất hết mod từ Vortex, nên quyết định tự làm ứng dụng này để quản lý mod inZOI tốt hơn và chủ động giữ dữ liệu của mình.

Đây là trải nghiệm cá nhân dẫn đến việc tạo dự án, không phải kết luận kỹ thuật về nguyên nhân mất dữ liệu hay khẳng định lỗi cụ thể của Vortex.

Ứng dụng desktop viết bằng **Svelte 5, Tauri 2 và Rust**, giao diện lấy cảm hứng từ Raycast.

## Định hướng

- Quản lý thư viện mod, thêm, sửa, gỡ và bật/tắt mod.
- Nhận diện mod inZOI đã được cài sẵn; phân biệt dữ liệu bên ngoài và mod do ứng dụng quản lý.
- Thumbnail, tìm kiếm nhanh và thao tác bàn phím.
- Nhận liên kết Nexus `nxm://inzoi/...` thường dùng bởi Vortex.
- Cập nhật ứng dụng từ GitHub Releases bằng gói có chữ ký.
- Build Windows và macOS.

## Phát triển

Cần Node.js 22+, Rust và công cụ build Tauri tương ứng với hệ điều hành (Visual Studio C++ Build Tools trên Windows, Xcode Command Line Tools trên macOS).

```sh
npm install
npm run desktop
```

```sh
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build
```

## Trạng thái đang phát triển

Dự án chưa có bản phát hành ổn định. Không xem các mục định hướng là chức năng đã được kiểm thử đầy đủ. Đăng ký giao thức `nxm` chỉ thực hiện khi người dùng chọn trong cài đặt vì nó thay ứng dụng mặc định nhận liên kết Vortex/Nexus.

Dữ liệu ứng dụng được lưu trong thư mục app-data riêng. Quét mod bên ngoài chỉ đọc. Gỡ mod trong thư viện chuyển vào `.trash` để giữ khả năng phục hồi. Bản sao mod tắt được giữ trong `.inzoimm-disabled` bên cạnh thư mục Mods trên cùng ổ đĩa.

## Phát hành

Repository dự kiến: `coolbirdzik/inzoi-better-mod-manager`. Cập nhật yêu cầu khóa ký Tauri và bản GitHub Release chứa `latest.json` cùng các gói được ký. Build macOS phải chạy trên máy macOS hoặc GitHub Actions macOS; không thể xác nhận build macOS chỉ bằng kiểm tra Windows.

## Tiến độ hiện tại

Đã triển khai: quét mod theo `mod_manifest.json`, thumbnail cục bộ nếu có, tìm kiếm/lọc, nhập thư mục hoặc ZIP vào thư viện riêng, tiếp nhận mod đã cài sau khi sao lưu, sửa tên/mô tả, bật/tắt bằng bản sao có đánh dấu quyền sở hữu, gỡ vào thư mục phục hồi, đọc và đăng ký liên kết NXM cho inZOI, kiểm tra/cài bản cập nhật đã ký.

Chưa hoàn tất: nhập 7z/RAR, legacy PAK/UE4SS, nhập dữ liệu Vortex cũ, kiểm thử toàn bộ luồng thao tác trên app thật và xác nhận bản macOS. Mod bên ngoài chỉ được bật/tắt, chỉnh sửa thông tin hoặc gỡ sau khi người dùng chọn Tiếp nhận quản lý; bản sao được giữ trong thư viện. Liên kết NXM được tiếp nhận ở cả lần mở app đầu tiên và khi app đã chạy.

Khóa ký cập nhật được tạo cục bộ ngoài repository. Để phát hành từ GitHub Actions, thêm private key vào secret `TAURI_SIGNING_PRIVATE_KEY`; không commit private key. Public key được cấu hình trong `src-tauri/tauri.conf.json`. Workflow tạo bản phát hành nháp; cần kiểm tra artifact trước khi công bố.


## Xác minh trên Windows

- Svelte/TypeScript: kiểm tra không có lỗi hoặc cảnh báo.
- Rust: 20 bài kiểm tra bao gồm chu trình tiếp nhận → tắt → bật → gỡ, bảo toàn bản sao, từ chối sai quyền sở hữu, ZIP có đường dẫn thoát thư mục, UTF-8 BOM, UTF-16 ở hai thứ tự byte và tên trường manifest viết camelCase.
- Đã mở bản Windows debug và quét thư mục mod thật; không thực hiện bật/tắt/gỡ mod thật trong quá trình thử giao diện.
- Chưa có bằng chứng build hoặc chạy macOS từ CI; workflow không thay thế kết quả chạy thực tế.


## Tải qua liên kết Nexus / Vortex

Trong Cài đặt, đăng ký nhận giao thức `nxm` trên Windows khi muốn thay ứng dụng đang nhận liên kết. Trên macOS, bản `.app` khai báo giao thức `nxm`; việc chọn ứng dụng mở liên kết do macOS/trình duyệt quyết định.

Nhấn Download with Manager trên Nexus hoặc dán liên kết NXM vào app. Nhập Nexus API key trong bảng liên kết. Khóa chỉ giữ trong bộ nhớ phiên, không ghi vào cấu hình, log hay Git. Tài khoản miễn phí cần liên kết còn hạn chứa `key` và `expires`; tài khoản Premium có thể yêu cầu link tải bằng API key. App lấy metadata và mirror từ API chính thức, dùng kết nối tải riêng không mang API key, và lưu file trong thư mục downloads riêng của app. ZIP có thể nhập ngay sau khi tải; 7z/RAR được giữ nguyên để không làm mất gói gốc.

Việc tải có xác thực thật chưa được kiểm chứng bằng tài khoản Nexus của người dùng. Kiểm tra tự động hiện bao phủ cấu trúc endpoint, tách thông tin xác thực khỏi dữ liệu gửi lên giao diện, và từ chối mirror không thuộc miền Nexus được hỗ trợ.

### Build Windows hiện có

Đã tạo bản cài NSIS **debug** tại `src-tauri/target/debug/bundle/nsis/inZOI Better Mod Manager_0.1.0_x64-setup.exe`. Đây là bản thử nghiệm; chưa phải release phát hành. Bản cài được tạo trước khi bổ sung luồng tải Nexus mới, nên cần build lại trước khi phân phối.
