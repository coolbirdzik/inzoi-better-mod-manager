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

Bản cài lưu dữ liệu trong app-data riêng; bản portable lưu trong thư mục `data` cạnh file chạy. Quét mod bên ngoài chỉ đọc. Gỡ mod trong thư viện chuyển vào `.trash` để giữ khả năng phục hồi. Bản sao mod tắt được giữ trong `.inzoimm-disabled` bên cạnh thư mục Mods trên cùng ổ đĩa.

## Phát hành

Repository: [coolbirdzik/inzoi-better-mod-manager](https://github.com/coolbirdzik/inzoi-better-mod-manager). Cập nhật yêu cầu khóa ký Tauri và bản GitHub Release chứa `latest.json` cùng các gói được ký. Build macOS phải chạy trên máy macOS hoặc GitHub Actions macOS; không thể xác nhận build macOS chỉ bằng kiểm tra Windows.

## Tiến độ hiện tại

Đã triển khai: quét mod theo `mod_manifest.json`, thumbnail cục bộ nếu có, tìm kiếm/lọc, nhập thư mục hoặc ZIP vào thư viện riêng, tiếp nhận mod đã cài sau khi sao lưu, sửa tên/mô tả, bật/tắt bằng bản sao có đánh dấu quyền sở hữu, gỡ vào thư mục phục hồi, đọc và đăng ký liên kết NXM cho inZOI, tự tải/cài bản cập nhật đã ký rồi khởi động lại.

Chưa hoàn tất: nhập 7z/RAR, legacy PAK/UE4SS, nhập dữ liệu Vortex cũ, kiểm thử toàn bộ luồng thao tác trên app thật và kiểm tra giao diện macOS trực tiếp. Mod bên ngoài chỉ được bật/tắt, chỉnh sửa thông tin hoặc gỡ sau khi người dùng chọn Tiếp nhận quản lý; bản sao được giữ trong thư viện. Liên kết NXM được tiếp nhận ở cả lần mở app đầu tiên và khi app đã chạy.

Khóa ký cập nhật được tạo cục bộ ngoài repository. Để phát hành từ GitHub Actions, thêm private key vào secret `TAURI_SIGNING_PRIVATE_KEY`; không commit private key. Public key được cấu hình trong `src-tauri/tauri.conf.json`. Workflow tạo bản nháp; bước xác minh tự động chỉ công bố khi đủ gói và tất cả chữ ký hợp lệ.


## Xác minh trên Windows

- Svelte/TypeScript: kiểm tra không có lỗi hoặc cảnh báo.
- Rust: 30 bài kiểm tra bao gồm chu trình tiếp nhận → tắt → bật → gỡ, bảo toàn bản sao, từ chối sai quyền sở hữu, ZIP có đường dẫn thoát thư mục, UTF-8 BOM, UTF-16 ở hai thứ tự byte và tên trường manifest viết camelCase.
- Đã mở bản Windows debug và quét thư mục mod thật; không thực hiện bật/tắt/gỡ mod thật trong quá trình thử giao diện.
- GitHub Actions đã build thành công DMG macOS cho mã nguồn 4dc3923 và lưu artifact `desktop-macOS`. Chưa kiểm tra giao diện macOS trực tiếp. Các thay đổi sau commit đó cần lượt kiểm tra mới.


## Tải qua liên kết Nexus / Vortex

Trong Settings (hoặc popup tải nếu chưa có key), nhấn **Get API key from Nexus / Lấy API key từ Nexus** để mở [trang API keys](https://www.nexusmods.com/settings/api-keys) bằng trình duyệt mặc định. Sao chép key rồi dán vào app. Key được dùng chung trong phiên hiện tại; đóng app sẽ xóa key khỏi bộ nhớ. Có nút xóa key ngay bên cạnh ô nhập. Không cần OAuth client ID hoặc đăng nhập OAuth trong app.

Trong Cài đặt, đăng ký nhận giao thức `nxm` trên Windows khi muốn thay ứng dụng đang nhận liên kết. Trên macOS, bản `.app` khai báo giao thức `nxm`; việc chọn ứng dụng mở liên kết do macOS/trình duyệt quyết định.

Nhấn **Download with Manager** trên Nexus: app hiện popup xác nhận, nhấn **OK** để đưa file vào hàng đợi. Nếu đã nhập API key trong phiên thì popup chỉ hiện thông tin file, nơi lưu và OK/Hủy. Nếu chưa có key, popup cho nhập key ngay. Có thể dán NXM thủ công trong **Downloads / Tải xuống** ở cuối thanh bên. Key chỉ giữ trong bộ nhớ phiên, không ghi vào cấu hình, log, Git hay kho thông tin xác thực. Tài khoản miễn phí cần liên kết còn hạn chứa `key` và `expires`; tài khoản Premium có thể yêu cầu link tải bằng API key. App lấy metadata và mirror từ API chính thức, dùng kết nối tải riêng không mang API key.

Trong **Downloads**, có thể xem tiến độ, tạm dừng các lượt đang chờ, hủy lượt đang tải/đang chờ, thử lại lỗi tải hoặc lỗi nhập, mở thư mục chứa file và dọn danh sách đã xong. Mỗi lần tải chạy nền, lần lượt từng file, không khóa Library. Hàng đợi thuộc phiên app hiện tại. Cập nhật tự động được hoãn khi còn lượt tải/nhập đang chờ hoặc đang chạy.

Chọn **Default download folder / Thư mục tải mặc định** trong Settings, hoặc nhấn **Change / Đổi** trong Downloads. Mặc định là `Downloads` bên trong thư mục Managed mods; mỗi file có thư mục riêng để tránh ghi đè. Thư mục tùy chọn được nhớ sau khi mở lại app. ZIP hợp lệ được giải nén ngay cạnh file nén trong thư mục tải và đăng ký vào **Managed mods**, giữ nguyên file nén và link/metadata Nexus; mod được thêm ở trạng thái chưa bật trong game. Nếu nhập lỗi, archive vẫn được giữ để thử nhập lại mà không tải lại. 7z/RAR được lưu nguyên gói và hiện rõ **cần giải nén**; sau khi giải nén có thể thêm bằng Add mod.


Việc tải có xác thực thật chưa được kiểm chứng bằng tài khoản Nexus của người dùng. Kiểm tra tự động hiện bao phủ cấu trúc endpoint, tách thông tin xác thực khỏi dữ liệu gửi lên giao diện, và từ chối mirror không thuộc miền Nexus được hỗ trợ.

### Windows portable và tự động cập nhật

Tải ZIP có tên `inZOI-Better-Mod-Manager_<version>_windows-x64-portable.zip` từ [Latest release](https://github.com/coolbirdzik/inzoi-better-mod-manager/releases/latest), giải nén vào thư mục có quyền ghi rồi chạy `inzoi-better-mod-manager.exe`. Giữ `portable.json` cạnh file chạy. Thư viện, cấu hình và tùy chọn ngôn ngữ nằm trong `data`; máy cần có WebView2 Runtime. Có thể đóng gói cục bộ sau khi build bằng `./scripts/package-portable.ps1 -Sign`.

App kiểm tra bản mới khi mở và mỗi 60 giây. Khi có bản mới, app tự tải, xác minh chữ ký, cài và khởi động lại, không cần nhấn xác nhận. App đợi thao tác mod hoặc hộp chỉnh sửa kết thúc trước khi cập nhật. Bản portable chỉ thay file chạy, giữ dữ liệu và bản file chạy trước đó trong `data/updates`; nếu thay file thất bại, app khôi phục file cũ. Bản cài Windows sử dụng bộ cài ở chế độ quiet. Windows vẫn có thể yêu cầu quyền quản trị nếu vị trí cài cần quyền đó.

Các bài kiểm tra giao diện mô phỏng kiểm tra bản mới, tự tải/cài/khởi động lại, lỗi chữ ký và trì hoãn lúc chỉnh sửa; kiểm tra Rust bao phủ thay file và khôi phục. Đã chạy helper cập nhật thật với gói thử được ký: chờ tiến trình cũ thoát, thay file, khởi động file mới, giữ dữ liệu và bản cũ; gói bị sửa một byte bị từ chối. Script lặp lại: `scripts/test-portable-update.ps1` (cần khóa ký trong biến môi trường). Chưa xác nhận trọn vòng cập nhật giữa hai bản GitHub Release trên app thật.

### Build Windows trước đây

Đã tạo bản cài NSIS **release** tại `src-tauri/target/release/bundle/nsis/inZOI Better Mod Manager_0.1.0_x64-setup.exe`, cùng chữ ký cập nhật `.exe.sig`. Chữ ký đã được kiểm tra với public key trong cấu hình app; thay đổi một byte của bộ cài làm việc kiểm tra thất bại. Bản release cục bộ chưa đồng nghĩa đã phát hành trên GitHub. Bản debug cũ vẫn nằm riêng trong `src-tauri/target/debug/bundle/nsis`.


### Công bố bản cập nhật

Workflow Release tạo bản nháp gồm bộ cài Windows x64, ZIP portable Windows x64, macOS Apple Silicon và macOS Intel. Sau khi cả ba job thành công, workflow Verify and publish release tải các asset của bản nháp, kiểm tra phiên bản và đủ nền tảng trong `latest.json`, rồi xác minh chữ ký từng gói bằng public key trong cấu hình của tag. Chỉ khi tất cả kiểm tra qua, bản nháp mới được công bố và đánh dấu Latest để app có thể kiểm tra cập nhật.

Công cụ kiểm tra độc lập ở `tools/verify-release`, không chạy các bộ cài được tải xuống. Nếu thiếu gói, sai URL, sai phiên bản hoặc sai chữ ký, workflow giữ nguyên bản nháp.

Bản nháp 0.1.1 không được công bố: kiểm tra kết nối cục bộ phát hiện thiếu khởi tạo TLS provider. Công cụ xác minh chặn bản này; bản sửa sẽ có phiên bản mới.

Bản sửa 0.1.2 khởi tạo rõ TLS provider trước các client HTTPS. Các kiểm tra truyền tải HTTP cục bộ đã qua; kiểm tra HTTPS chỉ đọc đã kết nối API Nexus thật và xác nhận lỗi khóa không hợp lệ được trả về an toàn. Đây chưa phải kiểm thử tải bằng API key thật của người dùng.


## Hiển thị mod

Trong **Managed mods → Choose managed mods folder**, chọn thư mục có sẵn rồi bật/tắt **Scan subfolders / Quét cả thư mục con** trong phần **Managed mods folder**. Lưu cài đặt sẽ nhận diện mod tại thư mục đó, giữ file tại chỗ và ghi nhận vào Managed mods. Quét lại không tạo bản ghi trùng; khi đổi thư mục hoặc tắt quét thư mục con, các mod đã quản lý vẫn được giữ. Mod mới nhận diện mặc định chưa triển khai vào game. Tùy chọn này không điều khiển quét thư mục game. Add mod vẫn có lựa chọn phạm vi trước khi nhập bản sao; nhập ZIP tự tìm mod trong gói.

Thư viện mặc định hiển thị dạng lưới với thumbnail; có thể chuyển sang danh sách bằng hai nút phía trên thư viện. Kiểu xem được ghi nhớ trên thiết bị. Chọn mod rồi nhấn **Choose thumbnail / Chọn ảnh thumbnail** trong bảng chi tiết để gán ảnh PNG, JPEG hoặc WebP (tối đa 20 MB), kể cả với mod cài sẵn. Ảnh được sao chép vào dữ liệu riêng của ứng dụng, giữ nguyên thư mục game và vẫn dùng được sau khi tiếp nhận quản lý mod. Mod thiếu ảnh hoặc ảnh không đọc được có hình thay thế.

Trên Windows, app tự đọc metadata CurseForge của game tại `%LOCALAPPDATA%/inZOI/User_data/88849/library.json` để tìm logo theo đúng ProjectName, rồi hiển thị ảnh từ `media.forgecdn.net`. Ảnh cục bộ và ảnh người dùng chọn được ưu tiên. Nguồn CurseForge cần kết nối mạng; mod không có metadata hoặc ảnh đi kèm vẫn cần chọn thumbnail thủ công.

## Ngôn ngữ giao diện

Tiếng Anh là mặc định, kể cả khi hệ điều hành đang dùng ngôn ngữ khác. Trong Settings → Language, có thể chọn English, Tiếng Việt, 简体中文, 한국어, 日本語, Français, Deutsch, Español, Português (Brasil) hoặc Русский. Thay đổi áp dụng ngay và được lưu trên thiết bị; không thay đổi dữ liệu mod.

Toàn bộ mục giao diện, hộp xác nhận, nhãn trợ năng và thông báo thông thường dùng catalog chung ở `src/lib/locales`. Chi tiết lỗi do hệ điều hành, dịch vụ hoặc backend trả về vẫn được giữ nguyên để hỗ trợ chẩn đoán. Tên, tác giả và mô tả mod là nội dung của mod, không tự dịch. Khóa dịch thiếu sẽ quay về tiếng Anh.

Kiểm tra: `npm run test:i18n` (đủ khóa và placeholder cho 10 ngôn ngữ). Kiểm tra giao diện chạy trên bản frontend đã build: `npm run build`, `npx playwright install chromium`, rồi `npm run test:ui`. Bộ kiểm tra xác nhận mặc định tiếng Anh, đổi Trung/Hàn tức thời và lưu lựa chọn qua lần tải lại. CI chạy cả hai bộ kiểm tra.

## Xác minh bản 0.1.3

Mã nguồn `83bd132`: [Verify desktop](https://github.com/coolbirdzik/inzoi-better-mod-manager/actions/runs/42296665839) thành công trên Windows và macOS; 30 bài kiểm tra Rust, 13 kiểm tra bản dịch và 7 kiểm tra giao diện qua. [Release](https://github.com/coolbirdzik/inzoi-better-mod-manager/actions/runs/42296676147) build thành công Windows x64 cùng macOS Intel/Apple Silicon. [Xác minh và công bố](https://github.com/coolbirdzik/inzoi-better-mod-manager/actions/runs/42298200068) thành công; [v0.1.3](https://github.com/coolbirdzik/inzoi-better-mod-manager/releases/tag/v0.1.3) đã công khai. Nguồn cập nhật `latest.json` trả phiên bản 0.1.3, gồm gói riêng `windows-x86_64-portable`.


## Nguồn thư mục, nhóm và tag

Trong **Managed mods**, chọn **Add source folders / Thêm thư mục nguồn**: popup hiện ngay **Scan subfolders / Quét thư mục con** ở phía trên, trước khi mở bộ chọn thư mục. Chọn chế độ quét, nhấn **Choose folder / Chọn thư mục**, rồi xác nhận thêm nguồn. Mỗi nguồn lưu tùy chọn quét riêng. Mở tab **Source folders / Thư mục nguồn** trong Managed mods để quản lý danh sách trên một màn hình riêng: tìm theo tên/đường dẫn, xem nhóm thư mục người dùng và thư mục ứng dụng, bật/tắt quét, mở vị trí trên ổ đĩa hoặc bỏ nguồn. Thư mục quản lý mặc định cũng có công tắc quét; thư mục Downloads được lấy từ nơi lưu file tải về. Menu chuột phải vẫn dùng được trên từng dòng. Tab **Mods / Mod** gom mod từ các nguồn tại chỗ cùng các mod đã nhập, giữ nguyên cách chia thư mục trên ổ đĩa. Bộ chọn nguồn trên thanh công cụ cho phép duyệt nhanh từng nguồn; danh sách quản lý nguồn không chiếm chỗ trong lưới mod. Sau khi thêm nguồn mới, app mở tab Thư mục nguồn để xem nguồn vừa thêm. Nút bỏ nguồn chỉ ngừng liệt kê nguồn, không xóa file. Nguồn lưu từ phiên bản cũ tự chuyển sang cấu hình mới với quét thư mục con bật sẵn.

**Downloads / Tải xuống** trong danh sách nguồn dùng đúng **Default download folder / Thư mục tải mặc định** người dùng đã chọn. Mỗi mod có một thư mục theo tên và ID Nexus; các file tùy chọn và phiên bản được tách theo file ID bên trong:

```text
Thư mục tải đã chọn/
  Linen Hair [nexus-123]/
    files/
      file-456/
        hair-v1.zip
        HairProject/
      file-789/
        hair-v2.zip
        HairProject/
```

Cùng mod ID luôn dùng lại thư mục đã có, kể cả khi tên trên Nexus thay đổi. Tải lại cùng file ID dùng lại archive đã lưu nếu kích thước và SHA-256 còn khớp; file bị sửa hoặc thiếu không bị ghi đè. Mỗi file có thư mục riêng nên các phiên bản không đè lên nhau; mỗi ProjectName chỉ bật được một bản trong game. ZIP được giải nén và đăng ký vào Managed mods tại chỗ; nhập lại cùng file giữ nguyên bản ghi và trạng thái hiện tại. Hủy hoặc lỗi tải chỉ dọn file tạm của lượt đó, giữ nguyên các bản đã lưu.

Đổi nơi tải chỉ áp dụng cho các lượt tải tiếp theo; các file đã tải/đang nhập vẫn giữ đúng vị trí của chúng. Cấu trúc thư mục tải cũ và các mod đã nhập vẫn được hỗ trợ, không tự di chuyển dữ liệu cũ. Thư mục mod đang cài của game xem ở Installed in game. Chọn một dòng nguồn để duyệt nhóm thư mục con; tìm kiếm trong nhóm bao gồm các thư mục con được quét.


Cột chi tiết có phần **Tags** để thêm tag (Enter) hoặc xóa từng tag. Tag hỗ trợ dấu cách, dùng được trong tìm kiếm và bộ lọc ở thanh trái, được lưu theo ProjectName trong cấu hình app và dùng chung cho các bản sao của cùng dự án. Tag vẫn giữ khi nhập hoặc nhận quản lý mod.

Phần **Metadata** hiển thị mọi trường đọc được từ `mod_manifest.json`, `meta.ini`, `metadata.json`, `nexus.json`, cùng thông tin tương ứng trong thư viện mod của game. Các liên kết HTTP/HTTPS tìm được được liệt kê thành nút mở; link NXM được chuyển thành trang Nexus công khai. Với ZIP tải qua Nexus bằng app, thông tin file và link nguồn được giữ lại khi nhập. App đọc dữ liệu cục bộ đã có; việc này không tự gọi API để tra cứu thêm mod.


## Điều hướng và menu chuột phải

Nút hông chuột Back/Forward, hai nút mũi tên trên thanh tìm kiếm và `Alt+Left` / `Alt+Right` quay lại/tiến tới vị trí đã duyệt trong app. Lịch sử giữ menu đang xem, đường dẫn thư mục, truy vấn tìm kiếm, bộ lọc tag, mod đang chọn và vị trí cuộn; đi tới vị trí mới sau khi quay lại sẽ xóa nhánh tiến tới cũ.

Chuột phải vào mod để mở thư mục, đổi ảnh, sửa tag và dùng các thao tác quản lý phù hợp với mod đó. Menu của nguồn cho phép mở, đổi chế độ quét hoặc bỏ nguồn; menu của thư mục có thao tác mở/duyệt. Chuột phải vào vùng trống để điều hướng, làm mới, thêm nguồn/mod hoặc đổi dạng hiển thị. Menu dùng được bằng `Shift+F10` / phím Menu, phím mũi tên, Enter và Escape. Ô nhập vẫn giữ menu chỉnh sửa văn bản mặc định.
