#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <d3d11.h>
#include <dxgi.h>
#include <math.h>

static LRESULT CALLBACK probe_window_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam)
{
    if (message == WM_DESTROY) { PostQuitMessage(0); return 0; }
    if (message == WM_KEYDOWN && wparam == VK_ESCAPE) { DestroyWindow(window); return 0; }
    return DefWindowProcW(window, message, wparam, lparam);
}

int luma_game_probe_run(void)
{
    HINSTANCE instance = GetModuleHandleW(NULL);
    WNDCLASSW window_class = {0};
    window_class.lpfnWndProc = probe_window_proc;
    window_class.hInstance = instance;
    window_class.hCursor = LoadCursorW(NULL, IDC_ARROW);
    window_class.lpszClassName = L"LumaNext.GameCaptureProbe.v1";
    RegisterClassW(&window_class);
    HWND window = CreateWindowExW(0, window_class.lpszClassName, L"Luma DX11 Game Capture Probe",
        WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT, 960, 540,
        NULL, NULL, instance, NULL);
    if (!window) return 2;

    DXGI_SWAP_CHAIN_DESC desc = {0};
    desc.BufferCount = 2;
    desc.BufferDesc.Width = 960;
    desc.BufferDesc.Height = 540;
    desc.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    desc.OutputWindow = window;
    desc.SampleDesc.Count = 1;
    desc.Windowed = TRUE;
    desc.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
    IDXGISwapChain *swap_chain = NULL;
    ID3D11Device *device = NULL;
    ID3D11DeviceContext *context = NULL;
    D3D_FEATURE_LEVEL feature_level;
    HRESULT result = D3D11CreateDeviceAndSwapChain(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0,
        NULL, 0, D3D11_SDK_VERSION, &desc, &swap_chain, &device, &feature_level, &context);
    if (FAILED(result)) { DestroyWindow(window); return 3; }
    ID3D11Texture2D *back_buffer = NULL;
    ID3D11RenderTargetView *target = NULL;
    result = swap_chain->lpVtbl->GetBuffer(swap_chain, 0, &IID_ID3D11Texture2D, (void **)&back_buffer);
    if (SUCCEEDED(result)) result = device->lpVtbl->CreateRenderTargetView(device, (ID3D11Resource *)back_buffer, NULL, &target);
    if (back_buffer) back_buffer->lpVtbl->Release(back_buffer);
    if (FAILED(result)) return 4;

    MSG message = {0};
    ULONGLONG started = GetTickCount64();
    while (message.message != WM_QUIT) {
        while (PeekMessageW(&message, NULL, 0, 0, PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
        float time = (float)(GetTickCount64() - started) / 1000.0f;
        float color[4] = {0.18f + 0.18f * sinf(time), 0.04f + 0.08f * sinf(time * 1.7f), 0.03f, 1.0f};
        context->lpVtbl->ClearRenderTargetView(context, target, color);
        swap_chain->lpVtbl->Present(swap_chain, 1, 0);
    }
    target->lpVtbl->Release(target);
    context->lpVtbl->Release(context);
    device->lpVtbl->Release(device);
    swap_chain->lpVtbl->Release(swap_chain);
    return 0;
}
