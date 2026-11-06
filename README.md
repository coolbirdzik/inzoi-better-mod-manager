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

Dữ liệu ứng dụng được lưu trong thư mục app-data riêng. Quét mod bên ngoài chỉ đọc. Gỡ mod trong thư viện chuyển vào `.trash` để giữ khả năng phục hồi. Bản sao mod tắt được giữ trong `.disabled`.

## Phát hành

Repository dự kiến: `coolbirdzik/inzoi-better-mod-manager`. Cập nhật yêu cầu khóa ký Tauri và bản GitHub Release chứa `latest.json` cùng các gói được ký. Build macOS phải chạy trên máy macOS hoặc GitHub Actions macOS; không thể xác nhận build macOS chỉ bằng kiểm tra Windows.

## Tiến độ hiện tại

Đã triển khai: quét mod theo `mod_manifest.json`, thumbnail cục bộ nếu có, tìm kiếm/lọc, nhập thư mục mod vào thư viện riêng, sửa tên/mô tả, bật/tắt bằng bản sao có đánh dấu quyền sở hữu, gỡ vào thư mục phục hồi, đọc và đăng ký liên kết NXM cho inZOI, kiểm tra/cài bản cập nhật đã ký.

Chưa hoàn tất: nhập ZIP/7z/RAR, legacy PAK/UE4SS, tải NXM có xác thực Nexus, chỉnh sửa/bật tắt trực tiếp mod bên ngoài, nhập dữ liệu Vortex cũ, kiểm thử toàn bộ luồng thao tác trên app thật và xác nhận bản macOS. Giao diện hiện chỉ cho sao chép mod bên ngoài vào thư viện, không chiếm quyền quản lý bản cài của game.

Khóa ký cập nhật được tạo cục bộ ngoài repository. Để phát hành từ GitHub Actions, thêm private key vào secret `TAURI_SIGNING_PRIVATE_KEY`; không commit private key. Public key được cấu hình trong `src-tauri/tauri.conf.json`. Workflow tạo bản phát hành nháp; cần kiểm tra artifact trước khi công bố.
