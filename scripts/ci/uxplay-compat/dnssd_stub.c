#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32) && !defined(EFI32) && !defined(EFI64)
#define DNSSD_STDCALL __stdcall
#else
#define DNSSD_STDCALL
#endif

typedef struct _DNSServiceRef_t *DNSServiceRef;
typedef uint32_t DNSServiceFlags;
typedef int32_t DNSServiceErrorType;

typedef void(DNSSD_STDCALL *DNSServiceRegisterReply)(
    DNSServiceRef sdRef,
    DNSServiceFlags flags,
    DNSServiceErrorType errorCode,
    const char *name,
    const char *regtype,
    const char *domain,
    void *context);

typedef union _TXTRecordRef_t {
    char PrivateData[16];
    char *ForceNaturalAlignment;
} TXTRecordRef;

typedef struct TxtRecordState {
    unsigned char *bytes;
    uint16_t length;
    uint16_t capacity;
    uint32_t reserved;
} TxtRecordState;

static TxtRecordState *txt_state(TXTRecordRef *txt_record) {
    return (TxtRecordState *)txt_record;
}

static const TxtRecordState *txt_state_const(const TXTRecordRef *txt_record) {
    return (const TxtRecordState *)txt_record;
}

__declspec(dllexport) DNSServiceErrorType DNSSD_STDCALL DNSServiceRegister(
    DNSServiceRef *sdRef,
    DNSServiceFlags flags,
    uint32_t interfaceIndex,
    const char *name,
    const char *regtype,
    const char *domain,
    const char *host,
    uint16_t port,
    uint16_t txtLen,
    const void *txtRecord,
    DNSServiceRegisterReply callBack,
    void *context) {
    (void)sdRef;
    (void)flags;
    (void)interfaceIndex;
    (void)name;
    (void)regtype;
    (void)domain;
    (void)host;
    (void)port;
    (void)txtLen;
    (void)txtRecord;
    (void)callBack;
    (void)context;
    return -65537;
}

__declspec(dllexport) void DNSSD_STDCALL DNSServiceRefDeallocate(DNSServiceRef sdRef) {
    (void)sdRef;
}

__declspec(dllexport) void DNSSD_STDCALL TXTRecordCreate(
    TXTRecordRef *txtRecord,
    uint16_t bufferLen,
    void *buffer) {
    (void)bufferLen;
    (void)buffer;
    TxtRecordState *state = txt_state(txtRecord);
    state->bytes = NULL;
    state->length = 0;
    state->capacity = 0;
    state->reserved = 0;
}

__declspec(dllexport) void DNSSD_STDCALL TXTRecordDeallocate(TXTRecordRef *txtRecord) {
    TxtRecordState *state = txt_state(txtRecord);
    free(state->bytes);
    state->bytes = NULL;
    state->length = 0;
    state->capacity = 0;
    state->reserved = 0;
}

__declspec(dllexport) DNSServiceErrorType DNSSD_STDCALL TXTRecordSetValue(
    TXTRecordRef *txtRecord,
    const char *key,
    uint8_t valueSize,
    const void *value) {
    TxtRecordState *state = txt_state(txtRecord);
    if (key == NULL) {
        return -65540;
    }

    size_t key_length = strlen(key);
    size_t entry_payload_length = key_length + 1u + (size_t)valueSize;
    if (entry_payload_length > 255u) {
        return -65540;
    }
    if ((size_t)UINT16_MAX - state->length < entry_payload_length + 1u) {
        return -65540;
    }

    uint16_t needed = (uint16_t)((size_t)state->length + entry_payload_length + 1u);
    if (needed > state->capacity) {
        uint16_t next_capacity = state->capacity == 0 ? 256 : state->capacity;
        while (next_capacity < needed) {
            if (next_capacity > UINT16_MAX / 2u) {
                next_capacity = UINT16_MAX;
                break;
            }
            next_capacity = (uint16_t)(next_capacity * 2u);
        }
        unsigned char *next = (unsigned char *)realloc(state->bytes, next_capacity);
        if (next == NULL) {
            return -65539;
        }
        state->bytes = next;
        state->capacity = next_capacity;
    }

    unsigned char *cursor = state->bytes + state->length;
    *cursor++ = (unsigned char)entry_payload_length;
    memcpy(cursor, key, key_length);
    cursor += key_length;
    *cursor++ = '=';
    if (valueSize > 0 && value != NULL) {
        memcpy(cursor, value, valueSize);
    }
    state->length = needed;
    return 0;
}

__declspec(dllexport) uint16_t DNSSD_STDCALL TXTRecordGetLength(const TXTRecordRef *txtRecord) {
    return txt_state_const(txtRecord)->length;
}

__declspec(dllexport) const void *DNSSD_STDCALL TXTRecordGetBytesPtr(const TXTRecordRef *txtRecord) {
    return txt_state_const(txtRecord)->bytes;
}
